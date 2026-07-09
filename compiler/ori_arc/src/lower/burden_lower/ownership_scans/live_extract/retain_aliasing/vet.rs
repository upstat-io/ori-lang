//! Gate (d) same-alloc closure growth + all-borrow-reads vetting for
//! [`super::compute_retain_aliasing_lineage`]: grows the closure over
//! `Let { Var }` aliases, niche-payload `Project` views, `Jump`-arg hops, AND
//! accessor-retain (`@unwrap`/`@get`) results, then vets every member use as a
//! borrow-read.

use rustc_hash::FxHashSet;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::super::super::is_provably_scalar_repr;

#[cfg(test)]
mod tests;

/// Gate (d) for the retain-aliasing consumer: grow the same-alloc closure over
/// `Let { Var }` aliases, niche-payload `Project` views, `Jump`-arg →
/// block-param hops, AND accessor-retain (`@unwrap`/`@get`) results (the
/// retain-aliasing payload view sharing the sum's allocation). Then vet every
/// member use as a borrow-read. `None` on any vet failure.
pub(super) fn retain_aliasing_closure_vetted(
    func: &ArcFunction,
    root: ArcVarId,
    accessor_retain_names: &FxHashSet<Name>,
) -> Option<FxHashSet<ArcVarId>> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if members.contains(src) && members.insert(*dst) => grew = true,
                    ArcInstr::Project { dst, value, .. }
                        if members.contains(value)
                            && !is_provably_scalar_repr(func, *dst)
                            && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    // Accessor-retain growth: a non-scalar `Apply` result whose
                    // callee self-increments a borrow-view of a member receiver
                    // is the SAME allocation.
                    ArcInstr::Apply {
                        dst,
                        func: callee,
                        args,
                        ..
                    } if accessor_retain_names.contains(callee)
                        && args.first().is_some_and(|a| members.contains(a))
                        && !is_provably_scalar_repr(func, *dst)
                        && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
            // Accessor-retain growth at a terminator-`Invoke` result.
            if let ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } = &block.terminator
            {
                if accessor_retain_names.contains(callee)
                    && args.first().is_some_and(|a| members.contains(a))
                    && !is_provably_scalar_repr(func, *dst)
                    && members.insert(*dst)
                {
                    grew = true;
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                for (pos, &arg) in args.iter().enumerate() {
                    if members.contains(&arg) {
                        if let Some(&(param, _)) = func.blocks[target.index()].params.get(pos) {
                            if members.insert(param) {
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
    retain_aliasing_member_uses_vetted(func, &members, accessor_retain_names).then_some(members)
}

/// Gate (d) vetting core: every member use is a borrow-read — a balanced
/// keep-alive `BurdenInc`/`BurdenDec`, a `Let`/`Project` closure hop, or an
/// `Apply`/`Invoke` member read at a borrowed position whose result is provably
/// scalar OR the accessor-retain payload view (a closure member). Any
/// owned-consume position / store / capture / `Set`/`Reuse`/`PartialApply`/
/// `ApplyIndirect` / non-`Invoke` terminator escape declines.
fn retain_aliasing_member_uses_vetted(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    accessor_retain_names: &FxHashSet<Name>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if !instr.used_vars().iter().any(|v| members.contains(v)) {
                continue;
            }
            match instr {
                // Balanced keep-alive RC ops + closure-internal alias/projection
                // hops are not consumes — they keep the lineage same-alloc.
                ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. }
                | ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. } => {}
                ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Select { .. }
                | ArcInstr::Reuse { .. }
                | ArcInstr::CollectionReuse { .. }
                | ArcInstr::PartialApply { .. }
                | ArcInstr::ApplyIndirect { .. } => return false,
                ArcInstr::Apply {
                    dst, func: callee, ..
                } => {
                    // A member at an owned position is a transfer out of family.
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                    // An `Apply` whose result `dst` is itself a member is a
                    // borrow-VIEW into the closure: vetted ONLY when the callee
                    // is accessor-retain (the result shares the payload alloc).
                    // A non-accessor-retain member-producing `Apply` is an
                    // un-vetted alias — decline. A borrowed read whose result is
                    // NEITHER a tracked member NOR provably scalar could alias
                    // the member into an UNTRACKED heap value (a sharing-view
                    // builtin — `slice`/`substring`/`take`/`drop`, which return
                    // a co-owning view over the SAME backing buffer — or an
                    // arbitrary user callee) that this per-reference placement
                    // cannot see; decline, mirroring the sibling
                    // `shared::member_uses_all_borrow_reads` scalar-result vet.
                    if members.contains(dst) {
                        if !accessor_retain_names.contains(callee) {
                            return false;
                        }
                    } else if !is_provably_scalar_repr(func, *dst) {
                        return false;
                    }
                }
                _ => {
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                }
            }
        }
        let term = &block.terminator;
        if !term.used_vars().iter().any(|v| members.contains(v)) {
            continue;
        }
        match term {
            ArcTerminator::Jump { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
            ArcTerminator::Invoke {
                dst, func: callee, ..
            } => {
                // A member at an owned `Invoke` arg = transfer — decline.
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return false;
                    }
                }
                // A member-producing terminator-`Invoke` is vetted ONLY when the
                // callee is accessor-retain (the result is the payload view). A
                // non-member, non-scalar result declines per the same
                // untracked-alias reasoning as the body `Apply` arm above.
                if members.contains(dst) {
                    if !accessor_retain_names.contains(callee) {
                        return false;
                    }
                } else if !is_provably_scalar_repr(func, *dst) {
                    return false;
                }
            }
            ArcTerminator::InvokeIndirect { .. }
            | ArcTerminator::Return { .. }
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. } => return false,
        }
    }
    true
}
