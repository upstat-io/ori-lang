//! NO-SINK death-point arm: the lineage has no dead-param sink and dies on the
//! borrowed-`Invoke` carrier's successor edges directly. Spec: Annex E §AIMS
//! RL-2 + RL-4.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

use super::super::super::successor_reachable_blocks;
use super::project_extract::has_live_project_extract;
use super::DeathPoint;

/// NO-SINK arm of [`super::choose_death_point`]: the carrier claim + the live
/// Project-extract decline gate. `None` when `allow_no_sink` is off or the
/// shape declines.
pub(super) fn choose_no_sink_death(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    allow_no_sink: bool,
    interner: &ori_ir::StringInterner,
) -> Option<DeathPoint> {
    if !allow_no_sink {
        return None;
    }
    let claim = choose_no_sink_carrier(func, members, interner)?;
    // Live Project-extract decline gate: `same_alloc_closure_vetted` does NOT
    // grow the closure to include `Project` results (a buffer element / niche
    // payload extracted from a member is a DISTINCT allocation the result-lineage
    // owns). Such a same-alloc view extracted from a member can be LIVE across the
    // carrier's successor edge where the per-edge release fires — a no-sink edge
    // release would then double-free the buffer the extract still holds. DECLINE
    // no-sink (dead-param mode only); a DECLINE, never a same_alloc closure union.
    // Spec: Annex E §AIMS RL-2.
    let carrier_block = carrier_block_of(func, members, claim)?;
    if has_live_project_extract(func, members, carrier_block) {
        return None;
    }
    Some(DeathPoint::NoSink { claim })
}

/// The block index whose may-unwind `Invoke`/`InvokeIndirect` terminator reads
/// `claim` (the carrier) at a borrowed arg position. `None` when not found
/// (declines the no-sink claim conservatively).
fn carrier_block_of(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    claim: ArcVarId,
) -> Option<usize> {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let term = &block.terminator;
        if !matches!(
            term,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if v == claim && members.contains(&v) && !term.is_owned_position(pos) {
                return Some(block_idx);
            }
        }
    }
    None
}

/// NO-SINK MODE: the lineage has no dead-param sink (the receiver
/// dies on the borrowed-`Invoke` carrier's successor edges directly). Returns
/// the carrier var to CLAIM for the class-ledger placement `deadAtSucc` per-edge
/// release.
///
/// # Carrier selection
///
/// The carrier is the closure member used at a BORROWED arg position of a
/// MAY-UNWIND `Invoke` / `InvokeIndirect` terminator that is the lineage's
/// EXECUTION-FINAL borrowed-`Invoke` read — every other member-read block
/// forward-reaches the carrier block, so the receiver's last read is the
/// carrier itself (a live-across receiver's later `.len()` borrow IS such a
/// later carrier, so the cure naturally walks to the post-call last read).
///
/// # Declines (`None`)
///
///  - no member is a borrowed may-unwind `Invoke` arg (no carrier),
///  - more than one borrowed-`Invoke` carrier block is execution-final (a fork
///    the single per-class per-edge release cannot disambiguate — conservatively
///    declined to avoid an under/over-release on a phi-merged shape),
///  - a member is read AFTER the chosen carrier on some path without itself
///    being a later borrowed-`Invoke` carrier (a non-carrier live-across use
///    the per-edge `deadAtSucc` release would phantom-suppress → leak; decline conservatively).
pub(in crate::lower::burden_lower::ownership_scans::borrowed_invoke_lineage) fn choose_no_sink_carrier(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> Option<ArcVarId> {
    let copy_arg_callees = crate::borrow::consuming_receiver_only_builtin_names(interner);
    // Carrier blocks: a block whose may-unwind `Invoke`/`InvokeIndirect`
    // terminator reads a member at a BORROWED arg position. Record (block_idx,
    // carrier_var).
    let mut carriers: Vec<(usize, ArcVarId)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let term = &block.terminator;
        let (ArcTerminator::Invoke {
            dst,
            normal,
            unwind,
            ..
        }
        | ArcTerminator::InvokeIndirect {
            dst,
            normal,
            unwind,
            ..
        }) = term
        else {
            continue;
        };
        // May-unwind requires a genuine edge split (the per-edge release
        // fires on the dying normal + unwind edges). A self-loop normal==unwind
        // is not a may-unwind carrier.
        if normal == unwind {
            continue;
        }
        // A heap-typed result may be a same-allocation VIEW of the carrier (a
        // slice): the view keeps the allocation live at the successor, the
        // per-edge probe suppresses the release on both edges, and the
        // suppressed inline pair releases nothing. A provably-Scalar result is
        // safe; so is a COPY-SEMANTICS builtin callee (`insert`/`remove`/
        // set-algebra per `CONSUMING_RECEIVER_ONLY_METHOD_NAMES`) whose
        // borrowed non-receiver args are elem-inc'd into the result, never held
        // as raw views. An unpopulated repr with an unknown callee declines
        // conservatively. Spec: Annex E §AIMS RL-2.
        let result_scalar = func
            .var_reprs
            .get(dst.index())
            .is_some_and(|r| *r == crate::ir::ValueRepr::Scalar);
        let copy_semantics_callee = matches!(
            term,
            ArcTerminator::Invoke { func: callee, .. } if copy_arg_callees.contains(callee)
        );
        if !result_scalar && !copy_semantics_callee {
            continue;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if members.contains(&v) && !term.is_owned_position(pos) {
                carriers.push((block_idx, v));
            }
        }
    }
    if carriers.is_empty() {
        return None;
    }
    // The execution-final carrier: a carrier block forward-reachable from EVERY
    // other member-read block (so the receiver's last read is this carrier).
    // Among the carriers, exactly one must be final; pick the carrier whose
    // block every member-read block reaches; require it unique.
    let member_read_blocks: Vec<usize> = func
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| {
            block
                .body
                .iter()
                .any(|i| i.used_vars().iter().any(|v| members.contains(v)))
                || block
                    .terminator
                    .used_vars()
                    .iter()
                    .any(|v| members.contains(v))
        })
        .map(|(idx, _)| idx)
        .collect();
    let mut final_carrier: Option<(usize, ArcVarId)> = None;
    for &(carrier_block, carrier_var) in &carriers {
        let is_final = member_read_blocks.iter().all(|&rb| {
            rb == carrier_block || successor_reachable_blocks(func, rb).contains(&carrier_block)
        });
        if is_final {
            if final_carrier.is_some() {
                // Two execution-final carriers — a fork the single per-class
                // per-edge release cannot disambiguate. Decline conservatively.
                return None;
            }
            final_carrier = Some((carrier_block, carrier_var));
        }
    }
    let (carrier_block, carrier_var) = final_carrier?;
    // (n1) the carrier block must not sit in a CFG cycle re-reaching itself (a
    // re-reached per-edge dec double-frees).
    if successor_reachable_blocks(func, carrier_block).contains(&carrier_block) {
        return None;
    }
    Some(carrier_var)
}
