//! Static RC coalescing peephole pass.
//!
//! Post-emission peephole that merges adjacent `RcInc`/`RcDec` operations
//! on the same variable within basic blocks. Runs after RC emission and
//! edge cleanup (Section 07.2).
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
//! - Function calls (`Apply`, `ApplyIndirect`) — callee may observe refcount
//! - Instructions that use or define the variable — refcount must be correct
//!   at the point of use (includes `IsShared`, `Set`, `Project`, etc.)
//!
//! # Complexity
//!
//! O(n) per block where n is the instruction count.

#[cfg(test)]
mod tests;

use rustc_hash::FxHashMap;

use crate::ir::{ArcInstr, ArcVarId, RcStrategy};

/// Pending RC operations for a single variable within a coalescing window.
struct PendingRc {
    incs: u32,
    decs: u32,
    strategy: RcStrategy,
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
            } => {
                accumulate_inc(&mut pending, &mut new_body, *var, *count, *strategy);
            }
            ArcInstr::RcDec { var, strategy } => {
                accumulate_dec(&mut pending, &mut new_body, *var, *strategy);
            }
            _ => {
                let is_call = matches!(
                    instr,
                    ArcInstr::Apply { .. } | ArcInstr::ApplyIndirect { .. }
                );

                if is_call {
                    // Calls are barriers for ALL variables — callee may
                    // observe any live variable's refcount (e.g., COW checks).
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
) {
    let entry = pending.entry(var).or_insert(PendingRc {
        incs: 0,
        decs: 0,
        strategy,
    });
    if entry.strategy == strategy {
        entry.incs += count;
    } else {
        // Strategy mismatch (same var, different strategy — shouldn't happen
        // but guard defensively). Flush existing and restart.
        flush_entry(out, var, entry);
        *entry = PendingRc {
            incs: count,
            decs: 0,
            strategy,
        };
    }
}

/// Accumulate an `RcDec` into the pending map.
fn accumulate_dec(
    pending: &mut FxHashMap<ArcVarId, PendingRc>,
    out: &mut Vec<ArcInstr>,
    var: ArcVarId,
    strategy: RcStrategy,
) {
    let entry = pending.entry(var).or_insert(PendingRc {
        incs: 0,
        decs: 0,
        strategy,
    });
    if entry.strategy == strategy {
        entry.decs += 1;
    } else {
        flush_entry(out, var, entry);
        *entry = PendingRc {
            incs: 0,
            decs: 1,
            strategy,
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
        });
    } else if entry.decs > entry.incs {
        for _ in 0..(entry.decs - entry.incs) {
            out.push(ArcInstr::RcDec {
                var,
                strategy: entry.strategy,
            });
        }
    }
    // Net zero: both cancelled — emit nothing.
}

/// Flush all pending RC operations (call barrier or block end).
fn flush_all(pending: &mut FxHashMap<ArcVarId, PendingRc>, out: &mut Vec<ArcInstr>) {
    // Sort by variable index for deterministic output ordering.
    let mut vars: Vec<_> = pending.keys().copied().collect();
    vars.sort_unstable();
    for var in vars {
        if let Some(entry) = pending.remove(&var) {
            flush_entry(out, var, &entry);
        }
    }
}
