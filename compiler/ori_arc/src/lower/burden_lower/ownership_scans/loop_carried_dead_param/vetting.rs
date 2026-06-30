//! Gate (c) lineage vetting for the RL-5 loop-carried dead-param scans: confirm
//! EVERY use of every same-alloc lineage member is a benign borrow-or-threading
//! appearance. The closure scan and the collection scan vet DIFFERENT benign-use
//! shapes (an `ApplyIndirect` borrow-receiver vs a COW-mutator owned receiver +
//! scalar `Project`), so the two vetters stay distinct; both are consumed as the
//! `vet` callback of the shared `compute_dead_param_lineage` skeleton.

use rustc_hash::FxHashSet;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::lower::burden_lower::is_provably_scalar_repr;

/// Closure-lineage vetting: true iff EVERY use of every member is a benign
/// borrow-or-threading appearance. Any owned-consume / store / capture /
/// projection / escape declines.
pub(super) fn closure_lineage_vetted(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches {
                continue;
            }
            match instr {
                // Alias-edge: the lineage's own internal hop.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // Borrow-receiver read: the member MUST be the closure receiver
                // (pos 0, always borrowed) and MUST NOT appear as an owned arg.
                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    if !members.contains(closure) {
                        return false;
                    }
                    if args.iter().any(|a| members.contains(a)) {
                        return false;
                    }
                }
                // Apply: a member may appear ONLY at a BORROWED arg position
                // (the callee borrows it). Any owned position declines.
                ArcInstr::Apply { args, .. } => {
                    if args
                        .iter()
                        .enumerate()
                        .any(|(i, a)| members.contains(a) && instr.is_owned_position(i))
                    {
                        return false;
                    }
                }
                // Any other instruction touching a member is an owned consume /
                // store / capture / projection — decline.
                _ => return false,
            }
        }
        // Terminator uses.
        let term = &block.terminator;
        match term {
            // Jump-arg threading: members feed successor block-params (grown
            // into the lineage). Benign.
            ArcTerminator::Jump { .. } => {}
            // Invoke: a member may appear ONLY at a BORROWED arg position.
            ArcTerminator::Invoke { args, .. } => {
                if args
                    .iter()
                    .enumerate()
                    .any(|(i, a)| members.contains(a) && term.is_owned_position(i))
                {
                    return false;
                }
            }
            // InvokeIndirect: the called closure (pos 0) borrows; member args
            // at owned positions decline.
            ArcTerminator::InvokeIndirect { closure, args, .. } => {
                if args.iter().any(|a| members.contains(a)) {
                    // arg positions in used_vars are offset by 1 (closure at 0).
                    let owned_arg = args
                        .iter()
                        .enumerate()
                        .any(|(i, a)| members.contains(a) && term.is_owned_position(i + 1));
                    if owned_arg {
                        return false;
                    }
                }
                let _ = closure;
            }
            // Return / Branch / Switch / Resume / Unreachable carrying a member
            // is an escape or a non-closure operand — decline.
            other => {
                if other.used_vars().iter().any(|v| members.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Collection-lineage vetting: true iff EVERY use of every member is a benign
/// borrow-or-threading appearance (an alias-edge, a BORROWED `Apply`/`Invoke`
/// arg, a `Jump`-arg threading edge, or the OWNED RECEIVER (pos 0) of a
/// COW-mutator whose result re-joins the lineage) OR the member is never used (a
/// wholly-dead thread). A member at any NON-receiver owned position (a value
/// moved INTO a collection, a non-COW owned consume, a store / capture /
/// projection / return) declines — a transferred or escaped lineage is a
/// different shape the base walk already balances.
pub(super) fn collection_lineage_vetted(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    cow_names: &FxHashSet<Name>,
) -> bool {
    // A member appears at an owned position OTHER than a COW-mutator's receiver
    // (pos 0) — an escape / value-into-collection / non-COW consume.
    let owned_escape =
        |callee: &Name, args: &[ArcVarId], is_owned: &dyn Fn(usize) -> bool| -> bool {
            let is_cow = cow_names.contains(callee);
            args.iter()
                .enumerate()
                .any(|(i, a)| members.contains(a) && is_owned(i) && !(is_cow && i == 0))
        };
    for block in &func.blocks {
        for instr in &block.body {
            if !instr.used_vars().iter().any(|v| members.contains(v)) {
                continue;
            }
            match instr {
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // A SCALAR-producing `Project` of a member is a benign field read
                // (the collection's length / capacity field, e.g. for index-bounds
                // math) — a borrow-view that carries no RC. A NON-scalar projection
                // (an owned sub-collection / element extract that could alias)
                // declines conservatively.
                ArcInstr::Project { dst, value, .. }
                    if members.contains(value) && is_provably_scalar_repr(func, *dst) => {}
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    if owned_escape(callee, args, &|i| instr.is_owned_position(i)) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        match &block.terminator {
            ArcTerminator::Jump { .. } => {}
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => {
                let term = &block.terminator;
                if owned_escape(callee, args, &|i| term.is_owned_position(i)) {
                    return false;
                }
            }
            other => {
                if other.used_vars().iter().any(|v| members.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}
