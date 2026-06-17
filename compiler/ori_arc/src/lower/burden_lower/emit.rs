//! Per-instruction + per-terminator burden-op emission for the Phase 5 walk.
//!
//! Drives the single-forward-pass per-block emission: `BurdenInc` before owned
//! positions + FRESH-allocating definitions, `BurdenDec` / `BurdenDecPartial` /
//! `BurdenDecVariant` / `BurdenDecField` at last-use / transfer / mutation
//! sites per `Spec: Annex E §AIMS RL-1 / RL-2 / TF-15`. Consumes the
//! precomputed analysis maps via `BurdenAnalysisCtx`; emits no lattice
//! consultation of its own.

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue};
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::instr_owned_position_transfer_vars;
use super::ownership_scans::ForwarderReleasePos;

/// Function-wide analysis results consumed by burden emission. Bundled into
/// one struct so per-instruction and per-terminator helpers share a single
/// reference — a domain newtype carrying the co-varying analysis fields at
/// their shared carrier.
pub(super) struct BurdenAnalysisCtx<'a> {
    pub(super) owned_vars_needing_rc: &'a FxHashSet<ArcVarId>,
    pub(super) last_uses_at: &'a FxHashMap<(usize, usize), Vec<ArcVarId>>,
    pub(super) full_move_vars: &'a FxHashSet<ArcVarId>,
    pub(super) partial_move_vars: &'a FxHashMap<ArcVarId, Vec<u32>>,
    // Vars whose paired BurdenDec is transfer-suppressed at their last-use
    // (transferred at last-use instr / terminator, or full-move). The
    // symmetric FRESH-site BurdenInc is suppressed for these in
    // emit_fresh_site_burden_inc to keep the per-value burden ledger balanced.
    pub(super) inc_suppressed_vars: &'a FxHashSet<ArcVarId>,
    // Let-Var alias dsts whose source stays live (duplication, RL-1). The burden
    // path emits the alias's own RcInc/RcDec pair: a `BurdenInc dst` at the alias
    // site (emit_fresh_site_burden_inc) balanced by a `BurdenDec dst` at the
    // alias's true last-use. The alias's release is the burden path's
    // responsibility, NOT the retiring predicate stack's.
    pub(super) dup_alias_dsts: &'a FxHashSet<ArcVarId>,
    // Move sources whose ownership transfers out through a Let-Var move-alias
    // chain to a terminator/owned-position transfer point (AIMS RL-2). Their
    // last-use BurdenDec is suppressed — the release is discharged at the
    // downstream transfer point, not at the move-alias site (else net -1).
    pub(super) transfer_via_move_alias: &'a FxHashSet<ArcVarId>,
    // Per-block live-out sets (owned-RC vars) per `Spec: Annex E §AIMS RL-4`.
    // A var live-out of block B has its in-block last-use `BurdenDec` suppressed
    // — the genuine release is the dead-out block's dec OR the predicate-stack
    // edge / dead-at-entry cleanup on the dying CFG edge, never an unconditional
    // dec in a block the value outlives.
    pub(super) live_out_per_block: &'a [FxHashSet<ArcVarId>],
    pub(super) contracts: &'a FxHashMap<Name, MemoryContract>,
    // Probe flag (`ORI_DISABLE_PREDICATE_STACK_RC=1`). When set, the predicate
    // stack RC emitter is off and the burden path is the sole real-RC emitter:
    // instruction-level transfer-suppression in `emit_last_use_decs` is lifted so
    // a value consumed at an owned instruction position (e.g. an `ApplyIndirect`
    // closure receiver) emits its release here instead of deferring to the
    // (disabled) predicate stack. Default-path (false) suppression is unchanged.
    pub(super) predicate_stack_rc_disabled: bool,
    // Vars consumed as a list-concat `PrimOp Binary(Add)` `RcPointer` operand
    // (`ori_list_concat_cow` dual-consuming contract). Precomputed before the
    // `&mut` emit walk (var reprs are stable), consulted by `emit_last_use_decs`
    // to suppress the spurious scope-exit `BurdenDec` whose double-dec with the
    // helper's internal consume frees the list buffer twice. SSOT for the
    // membership predicate is `instr_transfer_vars` (`burden_lower/mod.rs`).
    pub(super) list_concat_transfer_vars: &'a FxHashSet<ArcVarId>,
    // Step-1 COW-inc set (`compute_cow_inc_borrowed_aliases`): borrowed-param
    // aliases consumed at an owned RcPtr COW-mutation / `iter` position. These
    // vars are NOT in `owned_vars_needing_rc` (a borrowed alias carries no
    // scope-exit dec, RL-2), so `emit_owned_position_incs` / the terminator inc
    // path skip them — but RL-1 mandates a COW-inc on the duplicating use. The
    // `BurdenInc` is emitted at the owned position; step 2 emits the paired
    // freeing `BurdenDec` for COW-MUTATOR receivers (body after-call OR
    // normal+unwind successor edges, RL-4). Probe-only: empty on the default path.
    pub(super) cow_inc_borrowed_aliases: &'a FxHashSet<ArcVarId>,
    // COW-MUTATOR builtin names = `all_cow_method_names` MINUS `iter`. Step 2
    // releases a COW-inc'd receiver only when its consuming call is a COW
    // MUTATOR (the result is FRESH, nothing else holds the original). An `iter`
    // receiver's COW-inc is balanced by the runtime `ori_iter_drop`, never a
    // burden-dec. Empty on the default path (no COW-inc emitted).
    pub(super) cow_mutator_names: &'a FxHashSet<Name>,
    // Apply/Invoke result dsts whose callee transfers an owned arg THROUGH the
    // return (AIMS RL-1): the result aliases the transferred-in allocation, not
    // a fresh one, so its FRESH-site BurdenInc is elidable — emitting it
    // double-counts under sole-emitter Phase-7 lowering (net +1 leak). Gated to
    // `RcPointer`/`FatValue` results (an Aggregate's projected fields carry
    // their own decs). SSOT: `compute_transfer_through_return_results`.
    pub(super) transfer_through_return_results: &'a FxHashSet<ArcVarId>,
    // The function's OWN params whose `MemoryContract.transfers_through_return`
    // is set (the param flows to a `Return` terminator). Per AIMS RL-2 the
    // `Return` terminal use transfers ownership back to the caller, so the
    // callee MUST NOT emit a scope-exit `BurdenDec` on the param — the caller
    // decs the bound result variable. Suppressed in `emit_last_use_decs` +
    // `emit_terminator_burden_decs`. Params have no FRESH inc, so no symmetric
    // inc-suppression. SSOT: `compute_transfer_through_return_param_vars`.
    pub(super) transfer_through_return_param_vars: &'a FxHashSet<ArcVarId>,
    // Interned name of the `__index` protocol builtin. Its codegen
    // (`emit_list_index` / `emit_map_index`) self-increments the extracted
    // non-scalar result so the caller owns its reference; AIMS emits ONLY the
    // balancing dec at last-use (RL-2), never an inc. The burden path MUST
    // elide the FRESH-site BurdenInc on an `__index` result (RL-1 inc-elision:
    // the result's `+1` is supplied by codegen, not a duplication) — emitting
    // it double-counts under sole-emitter Phase-7 lowering (net +1 leak per
    // heap element index).
    pub(super) index_builtin_name: Name,
    // BORROWED-param-rooted vars consumed at an aggregate-STORE position
    // (`Construct` / `Reuse` / `CollectionReuse` arg, `Set.value`). The store
    // duplicates the caller's retained reference (RL-1), so a `BurdenInc` is
    // emitted BEFORE each store site consuming a member; no paired dec — the
    // container's drop is the matched release. SSOT:
    // `compute_borrowed_store_dup_args`.
    pub(super) borrowed_store_dup_args: &'a FxHashSet<ArcVarId>,
    // RL-1 genuine-duplication OWNED-CALL-ARG aliases (`Let { Var(src) }`
    // dup-aliases consumed at an owned call-arg position while `src` stays
    // live). Their alias-site `BurdenInc` is the fork's funded reference —
    // the consumer's release (COW copy-source dec / callee owned-param
    // release) is the matched release, so:
    //   - the inc is NOT tallied into `inc_counts` (escaping the
    //     `emit_terminator_burden_decs` symmetric same-block cancellation);
    //   - no last-use `BurdenDec` is emitted for the alias (suppressed in
    //     `emit_last_use_decs` + the terminator last-use path).
    // SSOT: `compute_funded_call_arg_dup_aliases`. Spec: Annex E §AIMS
    // RL-1 (`RL1_duplication_balanced`) + RL-2 (`RL2_transfer_kinds_no_dec`).
    pub(super) call_arg_dup_aliases: &'a FxHashSet<ArcVarId>,
}

/// Drive the unified single-forward-pass per-block emission. For each
/// instruction, `BurdenInc` is emitted BEFORE for every owned-position arg per
/// `ArcInstr::is_owned_position(pos)`; `BurdenDec` is emitted AFTER for each
/// last-use position EXCEPT when the instruction consumes the var at an owned
/// position (transfer point; ownership transferred per AIMS RL-2). `Set`/
/// `SetTag` carve-outs per AIMS TF-15 apply at both halves; `full_move_vars`
/// suppresses `BurdenDec` emission for vars whose entire owned-field set is
/// covered by `moved_out_fields`.
pub(super) fn emit_burden_ops_for_blocks(
    func: &mut ArcFunction,
    analysis: &BurdenAnalysisCtx<'_>,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    terminator_inc_per_block: &[Vec<ArcVarId>],
    // Per-block representative dead-forwarder block-param vars needing exactly ONE
    // RL-5 dead-at-entry release (deduped by forwarder-identity allocation). SSOT:
    // `compute_dead_forwarder_block_param_releases`. Emitted as a single
    // `BurdenDec(param)` at the START of the block's body (RL-5 immediate cleanup);
    // the param is dead (`Cardinality = Absent`) so the dec is its sole lifecycle
    // event in the block — `RL5_cleanup_balanced` (`[inc, dec]` nets 0) holds with
    // the predecessor Jump-arg handoff inc.
    dead_forwarder_param_releases: &FxHashMap<usize, Vec<ArcVarId>>,
    // Per-`(block, instr)` forwarder-RESULT release vars: a single whole-var
    // `BurdenDec(result_var)` emitted AFTER the lineage's last-use instruction for a
    // transfer-through-return forwarder result whose monomorphized result-type burden
    // is empty (so the result lineage was never collected into `owned_vars_needing_rc`).
    // RL-2 `RL2_borrowed_param_emits_caller_dec` + RL-34: the caller owns the returned
    // allocation and MUST release it at the borrow-read last use; the empty result-type
    // burden dropped the dec. The whole-var dec lowers (Phase 7) to a `RcDec` whose
    // drop-glue frees the result's owned fields (Aggregate Ok-payload) or the result
    // pointer (RcPtr buffer). SSOT: `compute_forwarder_result_under_release`. NOT tallied
    // into `inc_counts` — it balances the transferred-in allocation's lifecycle +1, not a
    // same-block inc. Keyed by `(block, ForwarderReleasePos)`: `BlockEntry` (normal
    // successor of a borrowed terminator-`Invoke`) or `AfterInstr(idx)` (after a body
    // borrow-`Apply`).
    forwarder_result_releases: &FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
) {
    // Per-block Inc count map for symmetric Dec emission at terminator-transfer
    // points. Populated DURING the emit walk so the Dec emission sees every Inc
    // actually pushed (FRESH-site Incs from `emit_fresh_site_burden_inc`,
    // instruction-level owned-position Incs from `emit_instr_burdens`, and
    // terminator-position Incs from `emit_terminator_burden_incs`). The
    // terminator Dec emission then emits one BurdenDec per Inc for vars whose
    // last-use is terminator-transferred, preserving VF-1 intraprocedural
    // balance. FRESH-site BurdenInc for Invoke/InvokeIndirect results is indexed
    // by the `normal` successor block where the result `dst` is bound.
    let invoke_result_incs = compute_invoke_result_incs(func, analysis);
    let invoke_result_dead_cleanup_decs = compute_invoke_result_dead_cleanup_decs(func, analysis);
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        let original = std::mem::take(&mut block.body);
        let terminator_idx = original.len();
        let mut new_body: Vec<ArcInstr> = Vec::with_capacity(original.len() * 2);
        let mut inc_counts: FxHashMap<ArcVarId, usize> = FxHashMap::default();
        // Prepend the Invoke-result FRESH-site Incs bound on this block's
        // normal-entry edge, before any body instruction.
        for &dst in &invoke_result_incs[block_idx] {
            new_body.push(ArcInstr::BurdenInc { var: dst });
            *inc_counts.entry(dst).or_insert(0) += 1;
        }
        // RL-2 cleanup dec for a DEAD (discarded) transfer-through-return Invoke
        // result bound on this block's normal-entry edge. The predecessor's
        // borrowed-call completed, the result IS the transferred-in allocation
        // (FRESH inc elided, caller arg dec suppressed), and it is used nowhere
        // (`let _ = id(args)`), so its sole release is this immediate dec. NOT
        // tallied into `inc_counts` — it balances the predecessor's
        // transferred-in lifecycle, not a same-block inc. Placed after the
        // Invoke-result FRESH incs so block-entry emission order is stable.
        for &dst in &invoke_result_dead_cleanup_decs[block_idx] {
            new_body.push(ArcInstr::BurdenDec { var: dst });
        }
        // RL-5 dead-at-entry cleanup for forwarder-identity allocations reaching this
        // block's DEAD block-params: exactly ONE `BurdenDec(param)` per distinct source
        // allocation (deduped by `compute_dead_forwarder_block_param_releases`). The
        // param entered with a live RC=1 reference (the predecessor Jump-arg handoff,
        // RL-4 exemption) that is used nowhere → its sole release is this immediate dec
        // (`AimsProof.Realization::RL5_dead_at_entry_cleanup` + `RL5_cleanup_balanced`).
        // Placed at block entry (before any body instruction) — the param is dead, so no
        // later use can precede the release. NOT tallied into `inc_counts`: this dec
        // balances the predecessor terminator's Jump-arg inc, not a same-block inc.
        if let Some(params) = dead_forwarder_param_releases.get(&block_idx) {
            for &param in params {
                new_body.push(ArcInstr::BurdenDec { var: param });
            }
        }
        // RL-2 forwarder-result release at the NORMAL-SUCCESSOR entry: the lineage's
        // final use was a BORROWED terminator-`Invoke` arg on a predecessor (e.g.
        // `Invoke @len(%10 [borrow]) normal bbN`). The borrowed call completed on that
        // terminator, so the result is dead at THIS block's entry — emit the single
        // whole-var `BurdenDec(result_var)` here, before any body instruction. NOT
        // tallied into `inc_counts` (balances the predecessor's transferred-in lifecycle,
        // not a same-block inc). Placed after the Invoke-result FRESH incs + dead-forwarder
        // releases so emission order within block entry is stable.
        if let Some(release_vars) =
            forwarder_result_releases.get(&(block_idx, ForwarderReleasePos::BlockEntry))
        {
            for &var in release_vars {
                new_body.push(ArcInstr::BurdenDec { var });
            }
        }
        for (instr_idx, instr) in original.into_iter().enumerate() {
            let ctx = BurdenEmitCtx {
                block_idx,
                instr_idx,
                analysis,
            };
            let before = new_body.len();
            emit_instr_burdens(&mut new_body, instr, &ctx);
            // Tally every BurdenInc the instruction emitted into this block.
            // A genuine-duplication call-arg alias's inc is NOT tallied — its
            // matched release is the consumer's, so the symmetric terminator
            // cancellation must not see it (RL-1 `RL1_duplication_balanced`).
            for emitted in &new_body[before..] {
                if let ArcInstr::BurdenInc { var } = emitted {
                    if analysis.call_arg_dup_aliases.contains(var) {
                        continue;
                    }
                    *inc_counts.entry(*var).or_insert(0) += 1;
                }
            }
            // RL-2 forwarder-result release: AFTER this instruction (the lineage's
            // last use was a body borrow-`Apply`), emit the single whole-var
            // `BurdenDec(result_var)` that the empty-result-type-burden gap dropped.
            // Placed after the last-use instr so the borrow-read completes before the
            // allocation is freed (no UAF). NOT tallied into `inc_counts` — it balances
            // the transferred-in allocation's lifecycle, not a same-block inc.
            if let Some(release_vars) = forwarder_result_releases
                .get(&(block_idx, ForwarderReleasePos::AfterInstr(instr_idx)))
            {
                for &var in release_vars {
                    new_body.push(ArcInstr::BurdenDec { var });
                }
            }
        }
        let before_term_incs = new_body.len();
        emit_terminator_burden_incs(&mut new_body, &terminator_inc_per_block[block_idx]);
        // Step-1 RL-1 COW-inc at terminator owned positions for borrowed-param
        // aliases (NOT in `owned_vars_needing_rc`, so absent from
        // `terminator_inc_per_block`). The paired freeing dec lands on the
        // normal-successor edge (step 2). `inc_counts` does NOT tally these — the
        // terminator-dec balancing path (`emit_terminator_burden_decs`) must NOT
        // emit a paired terminator dec for a borrowed-arg COW-inc (the borrowed
        // value survives the call; its release is the successor-edge dec, RL-4).
        emit_terminator_cow_incs(
            &mut new_body,
            &block.terminator,
            analysis.cow_inc_borrowed_aliases,
        );
        for emitted in &new_body[before_term_incs..] {
            if let ArcInstr::BurdenInc { var } = emitted {
                // Tally only the non-COW terminator incs (the COW-inc set is
                // balanced by the successor-edge dec, not a terminator dec).
                if !analysis.cow_inc_borrowed_aliases.contains(var) {
                    *inc_counts.entry(*var).or_insert(0) += 1;
                }
            }
        }
        // When the predicate stack is disabled (probe), the burden walk is the
        // sole RC emitter: it MUST emit the borrowed-Invoke-arg scope-exit dec
        // itself (the predicate stack's `release_with_burden_edge` is off).
        // Passing an empty borrowed-arg set un-suppresses those decs. On the
        // default path the predicate stack co-emits, so suppression stays.
        let empty_borrowed: FxHashSet<ArcVarId> = FxHashSet::default();
        let terminator_borrowed_args = if analysis.predicate_stack_rc_disabled {
            empty_borrowed
        } else {
            invoke_terminator_borrowed_args(&block.terminator)
        };
        emit_terminator_burden_decs(
            &mut new_body,
            block_idx,
            terminator_idx,
            analysis,
            &terminator_borrowed_args,
            &terminator_transfer_per_block[block_idx],
            &inc_counts,
        );
        block.body = new_body;
    }
    trace_burden_orphans(func, analysis.owned_vars_needing_rc);
}

/// Vars used at a BORROWED (non-owned) arg position of an `Invoke` /
/// `InvokeIndirect` terminator. A borrowed Invoke arg is NOT consumed by the
/// callee — the value survives to the `normal` / `unwind` successors, where the
/// predicate-stack edge cleanup (`emit_rc::release_with_burden`) releases it and
/// co-emits the paired scope-exit `BurdenDec`. Emitting the burden-walk's own
/// terminator-last-use `BurdenDec` for such an arg double-counts the release
/// (VF-1 net=-1 per terminal path). Used by `emit_terminator_burden_decs` to
/// suppress the redundant terminator dec. Non-Invoke terminators (Return /
/// Jump / Branch) return empty — their owned transfers are handled by
/// `terminator_transfer_vars` and their non-arg last-uses are genuine
/// scope-exit releases the burden walk owns.
fn invoke_terminator_borrowed_args(term: &ArcTerminator) -> FxHashSet<ArcVarId> {
    let mut borrowed: FxHashSet<ArcVarId> = FxHashSet::default();
    if matches!(
        term,
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
    ) {
        for (pos, &var) in term.used_vars().iter().enumerate() {
            if !term.is_owned_position(pos) {
                borrowed.insert(var);
            }
        }
    }
    borrowed
}

/// Emit `BurdenInc` for each owned terminator-position arg pre-computed by
/// `compute_terminator_inc_per_block`. Mirrors `emit_instr_burdens`'s
/// instruction-level `BurdenInc` loop — conservative Phase 5 emission at every
/// transfer point per AIMS RL-1; the lattice rewrite eliminates redundant Incs.
///
/// Lands BEFORE `emit_terminator_burden_decs` so the emitted IR sequence at
/// terminator position is `[terminator BurdenIncs] [terminator BurdenDecs]`
/// before the terminator itself (which lives in `block.terminator`, not
/// `block.body`). Decs suppress transfer vars per the existing transfer-set
/// gate; the symmetric Inc emission balances duplication arising from
/// multi-position-same-var terminators.
fn emit_terminator_burden_incs(new_body: &mut Vec<ArcInstr>, incs: &[ArcVarId]) {
    for &var in incs {
        new_body.push(ArcInstr::BurdenInc { var });
    }
}

/// Emit the step-1 RL-1 COW-inc for borrowed-param aliases consumed at an OWNED
/// arg position of an `Invoke` / `InvokeIndirect` terminator (the borrowed value
/// is COW-mutated by the callee — a duplicating use whose inc is not elidable).
/// The borrowed value SURVIVES the borrowed call's normal/unwind successors,
/// where step 2's edge cleanup emits the paired freeing `BurdenDec` (RL-4).
/// Non-Invoke terminators contribute no COW-inc — their owned transfers are
/// genuine ownership handoffs covered by `terminator_inc_per_block`.
fn emit_terminator_cow_incs(
    new_body: &mut Vec<ArcInstr>,
    term: &ArcTerminator,
    cow_inc_borrowed_aliases: &FxHashSet<ArcVarId>,
) {
    if !matches!(
        term,
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
    ) {
        return;
    }
    for (pos, &var) in term.used_vars().iter().enumerate() {
        if term.is_owned_position(pos) && cow_inc_borrowed_aliases.contains(&var) {
            new_body.push(ArcInstr::BurdenInc { var });
        }
    }
}

/// Read-only context bundle for per-instruction burden emission. Carries the
/// position (`block_idx`/`instr_idx`) plus the loop-invariant analysis maps
/// (`owned_vars_needing_rc`, `last_uses_at`, `full_move_vars`,
/// `partial_move_vars`) consumed by `emit_instr_burdens` per AIMS RL-2. Domain
/// newtype bundling the co-varying emission inputs.
struct BurdenEmitCtx<'a> {
    block_idx: usize,
    instr_idx: usize,
    analysis: &'a BurdenAnalysisCtx<'a>,
}

/// Emit `BurdenInc` ops before `instr`, push `instr` itself, then emit
/// `BurdenDec` ops at any last-use position for vars not consumed at an owned
/// position by this instruction. `Set` carve-outs (`value` is Owned via IA-5
/// alias-transfer despite `is_owned_position`'s `_ => false`) are applied
/// symmetrically per AIMS TF-15.
///
/// FRESH-allocating instructions (`Construct`, `PartialApply`, `Reuse`,
/// `CollectionReuse`, `Apply`/`Invoke` with Owned-return contract,
/// `Let { Literal::String }`) emit `BurdenInc dst` at definition site per AIMS
/// TF-3 / TF-5 / TF-6 / TF-7 / TF-9 / TF-9a ("FRESH starts Owned"), symmetric
/// with the scope-exit `BurdenDec` at last-use. Gated on
/// `owned_vars_needing_rc.contains(&dst)` per the coexistence handshake —
/// scalars naturally excluded per the `burden_carries_rc` filter.
fn emit_instr_burdens(new_body: &mut Vec<ArcInstr>, instr: ArcInstr, ctx: &BurdenEmitCtx<'_>) {
    // Owned-position Incs + in-place-mutation drops reference PRE-EXISTING
    // args, so they precede the instruction.
    emit_owned_position_incs(new_body, &instr, ctx);
    emit_in_place_mutation_drops(new_body, &instr, ctx);
    // RL-1 borrowed-store duplication incs follow the old-value drops so the
    // `Set` ordering invariant holds: `BurdenDecField` BEFORE
    // `BurdenInc(value)` BEFORE the mutation.
    emit_borrowed_store_dup_incs(new_body, &instr, ctx);
    let transfer_vars = instr_owned_position_transfer_vars(&instr);
    // The FRESH-site Inc references `dst`, DEFINED by `instr`. Under the probe
    // (`predicate_stack_rc_disabled`) the surviving whole-var burden ops lower
    // mechanically to real `RcInc`/`RcDec`, so a `BurdenInc dst` placed BEFORE
    // the defining `Construct` / `PartialApply` / `Reuse` / `Apply`-result /
    // dup-`Let` would lower to an `RcInc` on an undefined var (the ArcIrEmitter
    // rejects it) — emit the Inc AFTER the instruction so the lowered RcInc
    // sees a defined value. On the DEFAULT path the burden ops are accounting
    // markers consumed by Phase-6 elimination / the predicate-stack coexistence
    // (never lowered to real RC at this site), so the historical pre-instruction
    // placement is byte-identical and MUST be preserved to keep default-path
    // codegen unchanged.
    let fresh_inc_dst = fresh_site_burden_inc_dst(&instr, ctx);
    // Step-2 (RL-4 / scope-exit release of the step-1 COW-inc): a borrowed-alias
    // receiver consumed at an owned COW/iter position by THIS body instruction
    // got a `BurdenInc` (step 1, before the instr). The duplicate reference's
    // job — keeping rc ≥ 2 across the COW realloc / iterator-drop — ends once the
    // call returns; emit the paired freeing `BurdenDec` AFTER the instr. The
    // value is dead immediately after the consume (push → fresh COW result; iter
    // → opaque iterator handle), so the dec releases exactly the inc'd reference
    // (net 0 per `AimsProof.Realization::rcBalance`). Captured BEFORE `instr` is
    // moved into `new_body`. Invoke-TERMINATOR COW-inc'd args are NOT released
    // here — the value survives into the normal/unwind successors, released by
    // step-2 edge cleanup (RL-4).
    let cow_release_after: Vec<ArcVarId> = cow_inc_args_consumed_by_instr(&instr, ctx);
    if ctx.analysis.predicate_stack_rc_disabled {
        // Probe path: emit the FRESH-site Inc AFTER the defining instruction so
        // the lowered RcInc references a defined dst.
        new_body.push(instr);
        if let Some(dst) = fresh_inc_dst {
            new_body.push(ArcInstr::BurdenInc { var: dst });
        }
        for var in cow_release_after {
            new_body.push(ArcInstr::BurdenDec { var });
        }
    } else {
        // Default path: historical pre-instruction placement (burden ops are
        // codegen no-op markers here — byte-identical to keep AOT unchanged).
        if let Some(dst) = fresh_inc_dst {
            new_body.push(ArcInstr::BurdenInc { var: dst });
        }
        new_body.push(instr);
    }
    emit_last_use_decs(new_body, ctx, &transfer_vars);
}

/// Step-2 helper: COW-MUTATOR receivers consumed at an owned position of THIS
/// body instruction that are in the step-1 COW-inc set. Their step-1 `BurdenInc`
/// is paired with a freeing `BurdenDec` emitted AFTER the instruction (the COW
/// realloc produced a FRESH result; the duplicate reference to the original is
/// released, nothing else holds it).
///
/// Releases ONLY genuine COW-MUTATOR receivers (`push` / `insert` / `set` /
/// `remove` / `pop` / `sort` / `reverse` / `add` / `concat` / map+set COW) and
/// `Set` / `SetTag` in-place mutations. EXCLUDES `iter`: an `iter` receiver's
/// COW-inc is balanced by the runtime `ori_iter_drop` (`IterState::Drop` calls
/// `ori_buffer_rc_dec` on the held buffer), NOT by a burden-dec here — emitting
/// one would double-release the buffer the iterator still holds (use-after-free).
/// Empty on the default path (COW-inc set empty) and for non-COW instructions.
fn cow_inc_args_consumed_by_instr(instr: &ArcInstr, ctx: &BurdenEmitCtx<'_>) -> Vec<ArcVarId> {
    if ctx.analysis.cow_inc_borrowed_aliases.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<ArcVarId> = Vec::new();
    // Only a COW-MUTATOR `Apply` releases its receiver here; `iter` does not
    // (the iterator's `ori_iter_drop` releases the held buffer).
    if let ArcInstr::Apply {
        func: callee, args, ..
    } = instr
    {
        if ctx.analysis.cow_mutator_names.contains(callee) {
            if let Some(&recv) = args.first() {
                if ctx.analysis.cow_inc_borrowed_aliases.contains(&recv) {
                    out.push(recv);
                }
            }
        }
    }
    if let ArcInstr::Set { base, .. } | ArcInstr::SetTag { base, .. } = instr {
        if ctx.analysis.cow_inc_borrowed_aliases.contains(base) {
            out.push(*base);
        }
    }
    out
}

/// Emit `BurdenInc` for every owned-position arg of `instr` whose last-use is
/// NOT this instruction. Skip owned-position `BurdenInc` when the arg's
/// last-use is THIS instruction: the matching `BurdenDec` would be
/// transfer-suppressed per AIMS RL-2, producing a `Σ Inc - Σ Dec = +1` VF-1
/// imbalance in `aims/verify/burden_balance.rs`. Suppressing both Inc + Dec
/// keeps the coexistence handshake clean: vars whose physical RC is owned by
/// the `aims/realize/walk.rs` predicate-stack stay OUT of `func.burden_emitted`,
/// preventing `populate_class_covered` from spuriously suppressing
/// predicate-stack RC. Burden* are no-op codegen markers; the predicate-stack
/// realize walk owns the real codegen RC for transferred-out vars.
fn emit_owned_position_incs(
    new_body: &mut Vec<ArcInstr>,
    instr: &ArcInstr,
    ctx: &BurdenEmitCtx<'_>,
) {
    let last_use_at_this_instr: &[ArcVarId] = ctx
        .analysis
        .last_uses_at
        .get(&(ctx.block_idx, ctx.instr_idx))
        .map_or(&[], Vec::as_slice);
    for (pos, &arg) in instr.used_vars().iter().enumerate() {
        if instr.is_owned_position(pos) && ctx.analysis.owned_vars_needing_rc.contains(&arg) {
            if last_use_at_this_instr.contains(&arg) {
                continue;
            }
            new_body.push(ArcInstr::BurdenInc { var: arg });
        }
        // Step-1 RL-1 COW-inc: a borrowed-param alias consumed at an owned
        // COW-mutation position (NOT in `owned_vars_needing_rc`, so the normal
        // owned-position inc above skipped it). Always emit the inc — the
        // duplicating COW-mutation use re-reads the refcount, so it is never
        // last-use-elided (the value survives the call into the caller's hand).
        else if instr.is_owned_position(pos)
            && ctx.analysis.cow_inc_borrowed_aliases.contains(&arg)
        {
            new_body.push(ArcInstr::BurdenInc { var: arg });
        }
    }
    // `Set`'s `base` (owned in-place mutation receiver per TF-15) is excluded by
    // `is_owned_position`'s `_ => false`; emit its COW-inc explicitly.
    if let ArcInstr::Set { base, .. } = instr {
        if ctx.analysis.cow_inc_borrowed_aliases.contains(base) {
            new_body.push(ArcInstr::BurdenInc { var: *base });
        }
    }
}

/// Emit the RL-1 duplication `BurdenInc` for every BORROWED-param-rooted arg
/// consumed at an aggregate-STORE position of THIS instruction (`Construct` /
/// `Reuse` / `CollectionReuse` arg, `Set.value`) per
/// `compute_borrowed_store_dup_args`.
///
/// The borrow proves the caller retains a live reference across the call, so
/// the store creates a real SECOND reference: the inc supplies the reference
/// the store consumes (`AimsProof.Realization::RL1_duplication_balanced`); the
/// container's drop is the matched release. No paired dec is emitted here —
/// borrowed values carry no scope-exit release in this function (RL-2: the
/// caller emits the borrowed param's release). Emitted BEFORE the instruction
/// on BOTH paths: the member var is a pre-existing alias / param, so the
/// probe-path mechanical `BurdenInc -> RcInc` lowering sees a defined value.
/// `CollectionReuse`'s `old_var` (position 0) is a consumed token, not a
/// stored value — only buffer-stored args (positions 1..) are members, which
/// the scan guarantees by construction (it admits `args` only).
fn emit_borrowed_store_dup_incs(
    new_body: &mut Vec<ArcInstr>,
    instr: &ArcInstr,
    ctx: &BurdenEmitCtx<'_>,
) {
    if ctx.analysis.borrowed_store_dup_args.is_empty() {
        return;
    }
    let mut emit = |var: ArcVarId| {
        if ctx.analysis.borrowed_store_dup_args.contains(&var) {
            new_body.push(ArcInstr::BurdenInc { var });
        }
    };
    match instr {
        ArcInstr::Construct { args, .. }
        | ArcInstr::Reuse { args, .. }
        | ArcInstr::CollectionReuse { args, .. } => {
            for &arg in args {
                emit(arg);
            }
        }
        ArcInstr::Set { value, .. } => emit(*value),
        _ => {}
    }
}

/// Emit the old-value drop ops that precede an in-place mutation (`Set` /
/// `SetTag`) per AIMS TF-15 / TF-15a + RL-10.
///
/// `Set`: emit `BurdenDecField(base.field)` BEFORE the `Set` mutation when
/// `base` carries any burden. The codegen layer walks `Burden::owned_fields()`
/// to filter which field positions actually need a drop. Mirrors the symmetric
/// `BurdenInc(value)` — `BurdenInc` transfers ownership INTO the field,
/// `BurdenDecField` releases the prior value OUT. Ordering invariant:
/// `BurdenDecField` BEFORE `BurdenInc(value)` BEFORE `Set` — old release
/// precedes new acquire precedes mutation, so codegen can read the prior value
/// via `GEP`+load BEFORE the store clobbers it.
///
/// `SetTag`: whole-var pattern (NOT field-positional) — the tag change
/// invalidates ALL payload fields of the OLD variant. Emit `BurdenDecVariant`
/// BEFORE the `SetTag` so codegen can `GEP`+load the current discriminant +
/// dispatch the per-variant burden walk BEFORE the store clobbers the tag.
/// `SetTag` has no value operand (TF-15a backward demand is `(base, Once)`
/// only), so no symmetric `BurdenInc(value)` — parallel to `Set`'s
/// `BurdenDecField`, scoped to the whole variant per RL-10.
fn emit_in_place_mutation_drops(
    new_body: &mut Vec<ArcInstr>,
    instr: &ArcInstr,
    ctx: &BurdenEmitCtx<'_>,
) {
    if let ArcInstr::Set { base, field, value } = instr {
        if ctx.analysis.owned_vars_needing_rc.contains(base) {
            new_body.push(ArcInstr::BurdenDecField {
                base: *base,
                field: *field,
            });
        }
        if ctx.analysis.owned_vars_needing_rc.contains(value) {
            new_body.push(ArcInstr::BurdenInc { var: *value });
        }
    }
    if let ArcInstr::SetTag { base, .. } = instr {
        if ctx.analysis.owned_vars_needing_rc.contains(base) {
            new_body.push(ArcInstr::BurdenDecVariant { var: *base });
        }
    }
}

/// Emit the last-use `BurdenDec` / `BurdenDecPartial` for vars whose last-use
/// is at this instruction position, via the three-way branch per AIMS RL-2:
/// (a) suppress entirely when var is ownership-transferred at this instr OR
///     var's entire owned-field set was moved (full-move) OR var's ownership
///     transfers out through a move-alias chain (`transfer_via_move_alias`);
///     a duplication Let-Var alias (`dup_alias_dsts`) is NOT suppressed — its
///     RL-1 alias-site `BurdenInc` (`emit_fresh_site_burden_inc`) is balanced by
///     the last-use `BurdenDec` emitted here;
/// (b) emit `BurdenDecPartial { var, skip_fields }` when some-but-not-all owned
///     fields were moved via field-projection transfers (partial-move; codegen
///     walks `owned_fields` minus `skip_fields`);
/// (c) emit standard `BurdenDec { var }` for the no-projection conservative
///     baseline.
///
/// Instruction-level transfer suppression is preserved per the coexistence
/// handshake. The owned-position `BurdenInc` deposited by `emit_instr_burdens`
/// is a VF-1 accounting marker, NOT a real `RcInc`; codegen's predicate-stack
/// realize walk (consulting `class_covered`) owns the physical RC management for
/// vars consumed at instruction-level owned positions (`Apply`/`PartialApply`/
/// `Construct`/etc.). Adding a symmetric `BurdenDec` here would mark the var in
/// `func.burden_emitted`, propagate through `populate_class_covered`, and
/// suppress predicate-stack RC emission — causing real-world RC leaks observed
/// in `match_alias::test_closure_*` AOT tests. For VF-1 balance, the legacy
/// owned-position Inc/transfer-Dec pattern is rebalanced separately by
/// `emit_terminator_burden_decs` and by `eliminate_burden_ops` paired elision.
fn emit_last_use_decs(
    new_body: &mut Vec<ArcInstr>,
    ctx: &BurdenEmitCtx<'_>,
    transfer_vars: &FxHashSet<ArcVarId>,
) {
    let Some(last_use_vars) = ctx
        .analysis
        .last_uses_at
        .get(&(ctx.block_idx, ctx.instr_idx))
    else {
        return;
    };
    for &var in last_use_vars {
        if let Some(scan) = last_use_dec_suppressor(ctx, var, transfer_vars) {
            trace_dec_site(ctx.block_idx, ctx.instr_idx, var, ctx.analysis, Some(scan));
            continue;
        }
        trace_dec_site(ctx.block_idx, ctx.instr_idx, var, ctx.analysis, None);
        if let Some(skip_fields) = ctx.analysis.partial_move_vars.get(&var) {
            new_body.push(ArcInstr::BurdenDecPartial {
                var,
                skip_fields: skip_fields.clone(),
            });
        } else {
            new_body.push(ArcInstr::BurdenDec { var });
        }
    }
}

/// SSOT for the body last-use `BurdenDec` suppression decision: returns the
/// name of the scan that suppresses `var`'s scope-exit dec at this instruction,
/// or `None` when the dec is a genuine release the burden walk emits. Consumed
/// by `emit_last_use_decs` (suppress-or-emit) AND `trace_dec_site` (suppression
/// attribution) so the diagnostic can never drift from the actual decision.
/// Spec: Annex E §AIMS RL-2 / RL-4.
fn last_use_dec_suppressor(
    ctx: &BurdenEmitCtx<'_>,
    var: ArcVarId,
    transfer_vars: &FxHashSet<ArcVarId>,
) -> Option<&'static str> {
    // `transfer_vars` = consumed at THIS instruction's owned position. On the
    // default path the downstream owned-position RcDec (or predicate stack)
    // discharges the release, so suppress the dec here. Under the probe the
    // predicate stack is off and a value consumed at an owned position whose
    // callee BORROWS (no real transfer) must be released by the burden path —
    // the ApplyIndirect closure-receiver + borrowed-Apply-arg case. Lift only
    // this instruction-local suppression under the probe.
    if !ctx.analysis.predicate_stack_rc_disabled && transfer_vars.contains(&var) {
        return Some("instr_transfer");
    }
    // `transfer_via_move_alias` = the value genuinely moves downstream through
    // a Let-Var move-alias chain to a real transfer point; its release is
    // discharged THERE, not at the move-alias site (else a double dec on the
    // shared allocation). Suppressed on BOTH paths.
    if ctx.analysis.transfer_via_move_alias.contains(&var) {
        return Some("transfer_via_move_alias");
    }
    // RL-2 callee transfer-source-dec strip: a param that transfers through the
    // return (per the function's own `MemoryContract`) hands its allocation back
    // to the caller — a callee scope-exit dec double-releases it
    // (`AimsProof.Realization::RL2_transfer_kinds_no_dec` for the `Return`
    // `TerminalUse`). PROBE-ONLY: on the default path the `burden_dec` marker
    // drives `populate_class_covered` to suppress the predicate stack's OWN real
    // dec on this param, so removing it there un-covers the class and the
    // predicate stack emits a real dec → double-free.
    if ctx.analysis.predicate_stack_rc_disabled
        && ctx
            .analysis
            .transfer_through_return_param_vars
            .contains(&var)
    {
        return Some("transfer_through_return_param");
    }
    // `full_move_vars` — container drop emits the field-grain decs.
    if ctx.analysis.full_move_vars.contains(&var) {
        return Some("full_move");
    }
    // List-concat consume (`ori_list_concat_cow` dual-consuming): the helper
    // dec/frees the operand buffer. The consume IS the release; a scope-exit
    // `BurdenDec` would double-free the buffer.
    if ctx.analysis.list_concat_transfer_vars.contains(&var) {
        return Some("list_concat_transfer");
    }
    // RL-1 genuine-duplication call-arg alias: its last use is the owned
    // call-arg consume (body `Apply`) — the consumer's release is the matched
    // release for the kept alias-site inc, so no scope-exit dec here on EITHER
    // path (`RL2_transfer_kinds_no_dec`).
    if ctx.analysis.call_arg_dup_aliases.contains(&var) {
        return Some("call_arg_dup_alias");
    }
    // RL-4: a per-block last-use is a genuine release only when the var is dead
    // at block exit. Live-out vars are released on the dying CFG edge
    // (predicate-stack edge cleanup) or at the dead-out block — not here.
    if ctx
        .analysis
        .live_out_per_block
        .get(ctx.block_idx)
        .is_some_and(|s| s.contains(&var))
    {
        return Some("live_out");
    }
    None
}

/// Permanent suppression-attribution probe: records, per owned var at a planned
/// body last-use dec site, whether the `BurdenDec` was EMITTED (`suppressor =
/// None`) or which named scan SUPPRESSED it. Pairs with `trace_burden_orphans`
/// (the function-level net-balance check) to localize an orphaned FRESH-site
/// `BurdenInc` whose paired scope-exit dec a suppression scan wrongly removed.
/// Activate via `ORI_LOG=ori_arc::aims::realize=trace`; zero-cost otherwise.
fn trace_dec_site(
    block_idx: usize,
    instr_idx: usize,
    var: ArcVarId,
    analysis: &BurdenAnalysisCtx<'_>,
    suppressor: Option<&'static str>,
) {
    if !analysis.owned_vars_needing_rc.contains(&var) {
        return;
    }
    tracing::trace!(
        target: "ori_arc::aims::realize",
        block = block_idx,
        instr = instr_idx,
        var = var.index(),
        suppressor = suppressor.unwrap_or("EMITTED"),
        "burden dec_site suppression attribution"
    );
}

/// Permanent orphaned-burden detector: after the full per-block emit walk,
/// computes the per-var net (`#BurdenInc − #BurdenDec*`) over the realized body
/// and traces every owned var whose net != 0 — a positive net is an orphaned
/// FRESH-site `BurdenInc` (leak under sole-emitter Phase-7 lowering), a negative
/// net an orphaned dec (double-free). Counts every dec flavor
/// (`BurdenDec` / `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant`) as
/// a release of its `base`/`var`. Activate via
/// `ORI_LOG=ori_arc::aims::realize=trace`; zero-cost otherwise.
fn trace_burden_orphans(func: &ArcFunction, owned_vars_needing_rc: &FxHashSet<ArcVarId>) {
    if !tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
        return;
    }
    let mut net: FxHashMap<ArcVarId, i64> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var } => *net.entry(*var).or_insert(0) += 1,
                ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var, .. } => *net.entry(*var).or_insert(0) -= 1,
                ArcInstr::BurdenDecField { base, .. } => *net.entry(*base).or_insert(0) -= 1,
                _ => {}
            }
        }
    }
    for (var, n) in net {
        if n != 0 && owned_vars_needing_rc.contains(&var) {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                var = var.index(),
                net = n,
                kind = if n > 0 { "ORPHANED_INC" } else { "ORPHANED_DEC" },
                "burden net-balance orphan"
            );
        }
    }
}

/// Emit FRESH-site `BurdenInc dst` for instructions that define a
/// freshly-allocated owned value per AIMS TF-3 / TF-5 / TF-6 / TF-7 / TF-9 /
/// TF-9a, AND the duplication-alias `BurdenInc dst` for a `Let { Var(src) }`
/// whose `src` stays live (RL-1: a value duplicated to a still-live source is a
/// genuine new reference). Symmetric with the scope-exit `BurdenDec` at
/// last-use; both gated on `owned_vars_needing_rc.contains(&dst)` per the
/// coexistence handshake (scalars excluded by the `burden_carries_rc` filter in
/// `compute_owned_vars_needing_rc`).
///
/// FRESH-allocating definition sites:
///   (a) `Let { Literal::String(_) }` — heap-allocated str body.
///   (b) `Construct` — TF-3 FRESH (`Owned`, `Unique`, `BlockLocal`).
///   (c) `Apply` / `Invoke` with callee `ReturnContract.uniqueness ∈
///       {Unique, MaybeShared}` — TF-6 refined to callee's return shape.
///       Conservative for unknown callees (no contract) — emits the Inc;
///       balanced by the existing terminator/last-use `BurdenDec`.
///   (d) `PartialApply` — TF-7 FRESH(NonReusable).
///   (e) `Reuse` — TF-9 FRESH (inherited shape).
///   (f) `CollectionReuse` — TF-9a FRESH(CollectionBuffer).
///
/// Duplication-alias definition site (RL-1):
///   (g) `Let { Var(src) }` where `dst ∈ dup_alias_dsts` (`src` use-count ≥ 2,
///       so `src` stays live past the alias). The alias is a genuine
///       duplication of `src`'s allocation; the new reference owes a paired
///       `BurdenInc dst` here and `BurdenDec dst` at `dst`'s true last-use.
///       A MOVE-alias (`src` used exactly once) is NOT in `dup_alias_dsts` and
///       emits no inc — the existing `src` FRESH-site inc covers the lineage.
///
/// Other definitions (TF-1 scalar Literal, TF-2 MOVE-alias Var, TF-2a `PrimOp`,
/// TF-4 Project (Borrowed), TF-8 Select (alias-transfer), TF-10 `IsShared`
/// (scalar), TF-10a Reset (scalar)) emit no Inc. Scalars naturally drop out
/// via the `owned_vars_needing_rc` gate.
///
/// `Apply` / `Invoke` with no contract: conservative emission (treat as
/// `MaybeShared` return). Indirect calls (`ApplyIndirect` / `InvokeIndirect`)
/// have no callee identity, so their dst is treated as `MaybeShared` per AIMS
/// TF-5a / TF-6c — also emits the Inc when dst is in `owned_vars_needing_rc`.
fn fresh_site_burden_inc_dst(instr: &ArcInstr, ctx: &BurdenEmitCtx<'_>) -> Option<ArcVarId> {
    let dst = match instr {
        ArcInstr::Let {
            dst,
            value: ArcValue::Var(_),
            ..
        } if ctx.analysis.dup_alias_dsts.contains(dst) => *dst,
        ArcInstr::Let {
            dst,
            value: ArcValue::Literal(LitValue::String(_)),
            ..
        }
        | ArcInstr::Construct { dst, .. }
        | ArcInstr::PartialApply { dst, .. }
        | ArcInstr::Reuse { dst, .. }
        | ArcInstr::CollectionReuse { dst, .. } => *dst,
        ArcInstr::Apply { dst, func, .. } => {
            // RL-1 inc-elision: the `__index` protocol builtin's codegen
            // self-increments its extracted non-scalar result so the caller
            // owns its reference; AIMS emits ONLY the balancing dec at
            // last-use, never an inc. The result's `+1` is supplied by codegen
            // (not a duplication), so a FRESH-site BurdenInc here double-counts
            // under sole-emitter Phase-7 lowering (net +1 leak per heap
            // element). `__index` has no contract, so without this it falls to
            // the `None => *dst` conservative-inc arm below.
            if *func == ctx.analysis.index_builtin_name {
                return None;
            }
            // TF-6: when the callee has a known contract, gate on its
            // ReturnContract.uniqueness. For Unique / MaybeShared returns,
            // the callee hands an owned reference to the caller — caller
            // owes a BurdenDec at last-use, which the existing emission
            // already covers. The Inc here closes the inc/dec pair.
            // No contract: conservative — treat as MaybeShared (matches
            // TF-5's CONSERVATIVE default of MaybeShared).
            match ctx.analysis.contracts.get(func) {
                Some(c) => match c.return_info.uniqueness {
                    Uniqueness::Unique | Uniqueness::MaybeShared => *dst,
                    Uniqueness::Shared => return None,
                },
                None => *dst,
            }
        }
        ArcInstr::ApplyIndirect { dst, .. } => {
            // TF-5a: indirect calls have no callee identity; treated as
            // MaybeShared. Emit FRESH-site Inc to balance the last-use Dec.
            *dst
        }
        _ => return None,
    };
    // Suppress the FRESH-site Inc when dst is the result of a callee that
    // transfers an owned arg THROUGH the return (RL-1): the result IS the
    // transferred-in allocation, not a fresh one — the arg's own alloc supplied
    // the `+1`. A spurious result-inc here double-counts under sole-emitter
    // Phase-7 lowering (net +1 LEAK). SSOT:
    // `compute_transfer_through_return_results`.
    if ctx.analysis.transfer_through_return_results.contains(&dst) {
        return None;
    }
    // Suppress the FRESH-site Inc when dst is move-transferred into an owned
    // position: its paired BurdenDec is transfer-suppressed (RL-2), so emitting
    // the Inc would orphan it (VF-1 net=+1). The container's own drop owns the
    // released reference. Non-transferred fresh values keep the paired Inc+Dec.
    if ctx.analysis.owned_vars_needing_rc.contains(&dst)
        && !ctx.analysis.inc_suppressed_vars.contains(&dst)
    {
        Some(dst)
    } else {
        None
    }
}

/// Per-block entry `BurdenInc` list for FRESH-allocating `Invoke` /
/// `InvokeIndirect` results. A may-unwind call binds its result `dst` on the
/// `normal` successor edge, so its FRESH-site `BurdenInc` — the terminator
/// analogue of `emit_fresh_site_burden_inc`'s `Apply` / `ApplyIndirect` arms —
/// lands at the TOP of the `normal` successor block. Gated identically per AIMS
/// TF-6 / TF-6a / TF-6c: `Invoke` consults the callee
/// `ReturnContract.uniqueness` (`Unique` / `MaybeShared` emit; `Shared` skips;
/// no contract is conservative `MaybeShared`); `InvokeIndirect` is always
/// conservative. The `owned_vars_needing_rc` + `!inc_suppressed_vars` filter
/// mirrors the final push gate so a transfer-suppressed dst gets no orphan inc.
/// Result indexed by successor block index; consumed by
/// `emit_burden_ops_for_blocks`.
fn compute_invoke_result_incs(
    func: &ArcFunction,
    analysis: &BurdenAnalysisCtx<'_>,
) -> Vec<Vec<ArcVarId>> {
    let mut per_succ: Vec<Vec<ArcVarId>> = vec![Vec::new(); func.blocks.len()];
    for block in &func.blocks {
        let (dst, normal) = match &block.terminator {
            ArcTerminator::Invoke {
                dst,
                func: callee,
                normal,
                ..
            } => {
                // RL-1 inc-elision: an `__index` result is self-incremented by
                // codegen (`__index` lowers to may-unwind `ori_list_get`, so it
                // can be an Invoke). AIMS emits only the balancing dec — no
                // FRESH-site inc, else net +1 leak. Mirrors the `Apply` arm in
                // `fresh_site_burden_inc_dst`.
                if *callee == analysis.index_builtin_name {
                    continue;
                }
                let shared_return = matches!(
                    analysis.contracts.get(callee),
                    Some(c) if matches!(c.return_info.uniqueness, Uniqueness::Shared)
                );
                if shared_return {
                    continue;
                }
                (*dst, *normal)
            }
            ArcTerminator::InvokeIndirect { dst, normal, .. } => (*dst, *normal),
            _ => continue,
        };
        // RL-1: an Invoke result whose callee transfers an owned arg through the
        // return aliases the transferred-in allocation (not fresh) — its
        // result-inc is elidable (`compute_transfer_through_return_results`).
        if analysis.transfer_through_return_results.contains(&dst) {
            continue;
        }
        if analysis.owned_vars_needing_rc.contains(&dst)
            && !analysis.inc_suppressed_vars.contains(&dst)
        {
            if let Some(slot) = per_succ.get_mut(normal.index()) {
                slot.push(dst);
            }
        }
    }
    per_succ
}

/// Per-normal-successor-block RL-2 cleanup `BurdenDec`s for a DEAD (discarded)
/// transfer-through-return `Invoke` result. When `@f`'s callee transfers an
/// owned arg THROUGH the return (`@id(xs) = xs`), the result `dst` IS the
/// transferred-in allocation (not fresh — its FRESH inc is elided by
/// `transfer_through_return_results`, and the caller's arg dec was suppressed at
/// the owned transfer into the call). If that result is then DISCARDED (`let _ =
/// id(args)`, used nowhere), NOTHING releases the allocation — the caller's arg
/// dec was suppressed and the result carries no dec: +1 leak. Per AIMS RL-2
/// (UNUSED owned non-scalar → immediate cleanup dec) the dead discarded result
/// owes exactly ONE cleanup dec, placed at the normal-successor entry (after the
/// call completes). Proven net-0: scratch
/// `DeadTransferReturnResult.cure_correct_both_branches` (+ the live-result
/// negative pin: a result with ANY use already carries its own last-use dec, so
/// the cleanup fires ONLY on a genuinely dead result — else double-free).
fn compute_invoke_result_dead_cleanup_decs(
    func: &ArcFunction,
    analysis: &BurdenAnalysisCtx<'_>,
) -> Vec<Vec<ArcVarId>> {
    let used = super::ownership_scans::function_used_vars(func);
    let mut per_succ: Vec<Vec<ArcVarId>> = vec![Vec::new(); func.blocks.len()];
    for block in &func.blocks {
        let (dst, normal) = match &block.terminator {
            ArcTerminator::Invoke { dst, normal, .. }
            | ArcTerminator::InvokeIndirect { dst, normal, .. } => (*dst, *normal),
            _ => continue,
        };
        // ONLY a transfer-through-return result (its FRESH inc was elided, and
        // the transferred-in allocation's caller-side dec was suppressed) AND
        // genuinely DEAD (used nowhere — the discarded `let _` case). A used
        // result already carries its own last-use dec; firing here would
        // double-free (the scratch live-result negative pin).
        if !analysis.transfer_through_return_results.contains(&dst) {
            continue;
        }
        if used.contains(&dst) {
            continue;
        }
        if let Some(slot) = per_succ.get_mut(normal.index()) {
            slot.push(dst);
        }
    }
    per_succ
}

/// Terminator-position emission. Per AIMS RL-2, Return transfers ownership to
/// the caller — Return's `value` is a terminator-transfer point. Owned locals
/// whose terminator-position last-use is NOT transferred get `BurdenDec`
/// emitted immediately before the terminator.
///
/// Transfer-suppression preserves Dec emission for vars that received a
/// `BurdenInc` earlier in the block. The FRESH-site Inc emission + the
/// owned-position Inc emission both deposit Incs that need balancing Decs at the
/// transfer point to preserve VF-1's intraprocedural net-zero invariant in
/// `aims/verify/burden_balance.rs`. The Decs are TF-N/A metadata annotations in
/// `aims/realize/walk.rs` — they do NOT drive real `RcDec` emission; they exist
/// solely for VF-1 accounting. The realize walk preserves the transfer semantic
/// (no real `RcDec` on transferred-out values) by treating Burden* instructions
/// as transparent.
///
/// One `BurdenDec` per `BurdenInc` per var: the `inc_counts` map records every
/// `BurdenInc` the emit walk pushed for this block (FRESH-site, owned-position,
/// terminator-position), so multi-position-same-var terminators (e.g., Jump
/// with `args=[%v, %v]` to two Owned params) get matching multi-emit Decs.
fn emit_terminator_burden_decs(
    new_body: &mut Vec<ArcInstr>,
    block_idx: usize,
    terminator_idx: usize,
    analysis: &BurdenAnalysisCtx<'_>,
    terminator_borrowed_args: &FxHashSet<ArcVarId>,
    terminator_transfer_vars: &FxHashSet<ArcVarId>,
    inc_counts: &FxHashMap<ArcVarId, usize>,
) {
    // Emit symmetric Dec at the terminator for every Inc the block deposited on
    // a transferred-out var. Walk transfer_vars instead of last_uses_at because
    // some vars receive Inc but are NOT in last_uses_at at terminator_idx — a
    // var with a FRESH-Inc at definition whose last-use is the Return terminator
    // is in BOTH last_uses_at AND terminator_transfer_vars, so a last_uses_at
    // walk would `continue` and emit no Dec.
    for &var in terminator_transfer_vars {
        let inc_count = inc_counts.get(&var).copied().unwrap_or(0);
        if inc_count == 0 {
            continue;
        }
        let dec_template = if let Some(skip_fields) = analysis.partial_move_vars.get(&var) {
            ArcInstr::BurdenDecPartial {
                var,
                skip_fields: skip_fields.clone(),
            }
        } else {
            ArcInstr::BurdenDec { var }
        };
        for _ in 0..inc_count {
            new_body.push(dec_template.clone());
        }
    }
    // Vars whose terminator-position last-use is NOT a transfer point follow the
    // last-use emission path: emit one BurdenDec per last-use entry unless
    // full_move OR move-alias-transfer suppresses. A duplication Let-Var alias
    // (`dup_alias_dsts`) is NOT suppressed: its RL-1 alias-site `BurdenInc`
    // (emit_fresh_site_burden_inc) is balanced by this last-use `BurdenDec`
    // (net 0). A MOVE-alias source transferring out is in `full_move_vars` or
    // `transfer_via_move_alias` and stays suppressed.
    if let Some(last_use_vars) = analysis.last_uses_at.get(&(block_idx, terminator_idx)) {
        for &var in last_use_vars {
            if terminator_transfer_vars.contains(&var) {
                continue;
            }
            if analysis.full_move_vars.contains(&var) {
                continue;
            }
            if analysis.transfer_via_move_alias.contains(&var) {
                continue;
            }
            // RL-1 genuine-duplication call-arg alias consumed at THIS
            // terminator (a contract-Owned user-callee arg whose Phase-5
            // `arg_ownership` annotation is still the borrowed default, so it
            // is not in `terminator_transfer_vars`): the consumer's
            // owned-param release is the matched release for the kept
            // alias-site inc — a terminator dec here would cancel it
            // (`RL2_transfer_kinds_no_dec`).
            if analysis.call_arg_dup_aliases.contains(&var) {
                continue;
            }
            // RL-2 callee transfer-source-dec strip: a param flowing to Return
            // transfers ownership to the caller — no callee scope-exit dec.
            // PROBE-ONLY (the default-path coexistence handshake relies on the
            // marker; see `emit_last_use_decs`).
            if analysis.predicate_stack_rc_disabled
                && analysis.transfer_through_return_param_vars.contains(&var)
            {
                continue;
            }
            // RL-4: a terminator-position last-use whose var is live-out of the
            // block is not a genuine release — the value flows on to a
            // successor (the dec belongs on the dying edge / dead-out block).
            if analysis
                .live_out_per_block
                .get(block_idx)
                .is_some_and(|s| s.contains(&var))
            {
                continue;
            }
            // Borrowed Invoke arg: the value survives the borrowed call and is
            // released at the successor by the predicate-stack edge cleanup,
            // which co-emits the paired scope-exit BurdenDec. A terminator dec
            // here too double-counts (VF-1 net=-1 per terminal path).
            if terminator_borrowed_args.contains(&var) {
                continue;
            }
            if let Some(skip_fields) = analysis.partial_move_vars.get(&var) {
                new_body.push(ArcInstr::BurdenDecPartial {
                    var,
                    skip_fields: skip_fields.clone(),
                });
            } else {
                new_body.push(ArcInstr::BurdenDec { var });
            }
        }
    }
}
