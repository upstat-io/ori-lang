//! Transitional ownership-operation coalescing peephole pass.
//!
//! Post-emission peephole that merges adjacent logical `RcInc`/`RcDec`
//! operations on the same variable within basic blocks. The shared carrier
//! still contains compiled-shaped `RcStrategy` and `RcAtomicity` fields, so
//! this compatibility pass treats both as exact fences and preserves them.
//! Production AIMS coalesces only logical event identities; any later physical
//! coalescing belongs after layout selection in the VM or compiled planner.
//!
//! # Patterns
//!
//! - `RcInc(x, a); RcInc(x, b)` → `RcInc(x, a+b)` (batched increment)
//! - `RcInc(x); RcDec(x)` with no intervening use → cancelled (net zero)
//! - `RcInc(x, 2); RcDec(x)` → `RcInc(x, 1)` (net positive)
//! - `RcDec(x); RcDec(x)` with no barrier → single net flush
//!
//! # Barriers
//!
//! RC operations are NOT coalesced across:
//! - Function calls (`Apply`, `ApplyIndirect`) — the callee may observe logical
//!   sharing state
//! - Instructions that use or define the variable — logical event order must
//!   be preserved (includes `IsShared`, `Set`, `Project`, etc.)
//!
//! # Complexity
//!
//! O(n) per block where n is the instruction count.

#[cfg(test)]
mod tests;

use rustc_hash::FxHashMap;

use crate::ir::{ArcInstr, ArcVarId, RcAtomicity, RcStrategy};

/// Pending RC operations for a single variable within a coalescing window.
struct PendingRc {
    incs: u32,
    decs: u32,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
}

/// Coalesce adjacent RC operations within a block's instruction list.
///
/// Scans instructions linearly, accumulating RC deltas per variable.
/// When a barrier is reached (call, use, or definition of the variable),
/// the accumulated delta is flushed as a single net operation.
pub(crate) fn coalesce_block_rc(body: &mut Vec<ArcInstr>) {
    if body.len() < 2 {
        return;
    }

    let old_body = std::mem::take(body);
    let mut new_body = Vec::with_capacity(old_body.len());
    let mut pending: FxHashMap<ArcVarId, PendingRc> = FxHashMap::default();

    for instr in old_body {
        match &instr {
            ArcInstr::RcInc {
                var,
                count,
                strategy,
                atomicity,
            } => {
                accumulate_inc(
                    &mut pending,
                    &mut new_body,
                    *var,
                    *count,
                    *strategy,
                    *atomicity,
                );
            }
            ArcInstr::RcDec {
                var,
                strategy,
                atomicity,
            } => {
                accumulate_dec(&mut pending, &mut new_body, *var, *strategy, *atomicity);
            }
            _ => {
                let is_call = matches!(
                    instr,
                    ArcInstr::Apply { .. } | ArcInstr::ApplyIndirect { .. }
                );

                if is_call {
                    // Calls are barriers for all variables because a callee may
                    // observe or change live logical sharing state.
                    flush_all(&mut pending, &mut new_body);
                } else {
                    // Flush pending ops for variables touched by this instruction.
                    flush_touched(&instr, &mut pending, &mut new_body);
                }

                new_body.push(instr);
            }
        }
    }

    // Flush remaining pending operations at block end.
    flush_all(&mut pending, &mut new_body);

    *body = new_body;
}

/// Accumulate an `RcInc` into the pending map.
fn accumulate_inc(
    pending: &mut FxHashMap<ArcVarId, PendingRc>,
    out: &mut Vec<ArcInstr>,
    var: ArcVarId,
    count: u32,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
) {
    let entry = pending.entry(var).or_insert(PendingRc {
        incs: 0,
        decs: 0,
        strategy,
        atomicity,
    });
    if entry.strategy == strategy && entry.atomicity == atomicity {
        entry.incs += count;
    } else {
        // Transitional physical mechanisms are exact coalescing fences.
        // Combining them would silently change the selected layout plan.
        flush_entry(out, var, entry);
        *entry = PendingRc {
            incs: count,
            decs: 0,
            strategy,
            atomicity,
        };
    }
}

/// Accumulate an `RcDec` into the pending map.
fn accumulate_dec(
    pending: &mut FxHashMap<ArcVarId, PendingRc>,
    out: &mut Vec<ArcInstr>,
    var: ArcVarId,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
) {
    let entry = pending.entry(var).or_insert(PendingRc {
        incs: 0,
        decs: 0,
        strategy,
        atomicity,
    });
    if entry.strategy == strategy && entry.atomicity == atomicity {
        entry.decs += 1;
    } else {
        flush_entry(out, var, entry);
        *entry = PendingRc {
            incs: 0,
            decs: 1,
            strategy,
            atomicity,
        };
    }
}

/// Flush pending RC operations for variables touched by a non-RC instruction.
fn flush_touched(
    instr: &ArcInstr,
    pending: &mut FxHashMap<ArcVarId, PendingRc>,
    out: &mut Vec<ArcInstr>,
) {
    for var in instr.used_vars() {
        if let Some(entry) = pending.remove(&var) {
            flush_entry(out, var, &entry);
        }
    }
    if let Some(dst) = instr.defined_var() {
        if let Some(entry) = pending.remove(&dst) {
            flush_entry(out, dst, &entry);
        }
    }
}

/// Emit the net RC effect for a single variable.
fn flush_entry(out: &mut Vec<ArcInstr>, var: ArcVarId, entry: &PendingRc) {
    if entry.incs > entry.decs {
        out.push(ArcInstr::RcInc {
            var,
            count: entry.incs - entry.decs,
            strategy: entry.strategy,
            atomicity: entry.atomicity,
        });
    } else if entry.decs > entry.incs {
        for _ in 0..(entry.decs - entry.incs) {
            out.push(ArcInstr::RcDec {
                var,
                strategy: entry.strategy,
                atomicity: entry.atomicity,
            });
        }
    }
    // Net zero: both cancelled — emit nothing.
}

/// Flush all pending RC operations (call barrier or block end).
///
/// Emits all net-Inc entries before all net-Dec entries; within each phase,
/// variables sort by index for deterministic output ordering.
///
/// # Inc-before-Dec invariant
///
/// BUG-04-090 F-prj + E-mat: a net-Inc on `Y` and a net-Dec on `X` where
/// `Dec(X)`'s field-walk owns `Y`'s allocation (e.g. `RcDec b [AggFields]`
/// walking `b.value` just projected as `Y`) would free `Y` before `Inc(Y)`
/// fires — a use-after-free at `Y`'s next use. Sound because: (1) `flush_all`
/// only fires at end-of-block/call-barriers, no intervening body instructions;
/// (2) unrelated variables are RC-neutral under either ordering; (3) aliased
/// variables need Inc-then-Dec to keep the allocation alive through the dec.
fn flush_all(pending: &mut FxHashMap<ArcVarId, PendingRc>, out: &mut Vec<ArcInstr>) {
    let mut inc_vars: Vec<ArcVarId> = Vec::new();
    let mut dec_vars: Vec<ArcVarId> = Vec::new();
    for (&var, entry) in pending.iter() {
        if entry.incs > entry.decs {
            inc_vars.push(var);
        } else if entry.decs > entry.incs {
            dec_vars.push(var);
        }
        // Net-zero entries flush to nothing — partition skips them.
    }
    inc_vars.sort_unstable();
    dec_vars.sort_unstable();

    for var in inc_vars {
        if let Some(entry) = pending.remove(&var) {
            flush_entry(out, var, &entry);
        }
    }
    for var in dec_vars {
        if let Some(entry) = pending.remove(&var) {
            flush_entry(out, var, &entry);
        }
    }
    // Drain any net-zero entries silently (flush_entry is a no-op for them).
    pending.clear();
}
