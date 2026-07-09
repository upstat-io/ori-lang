//! Shared root/vetting utilities consumed by both [`super`]'s fresh-sum
//! live-extract scan and the [`super::retain_aliasing`] sibling scan (and,
//! externally, by [`super::super::sum_payload_iter_consume`]).

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};

use crate::lower::burden::{Burden, TypeRef};
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use super::super::super::is_provably_scalar_repr;

/// Candidate FRESH-sum roots per gate (a): sum-aggregate `Construct` dsts +
/// `Apply` / `Invoke` results whose callee contract hands the caller an owned
/// reference (`uniqueness ∈ {Unique, MaybeShared}`) without transferring an
/// arg through the return.
pub(in crate::lower::burden_lower) fn collect_fresh_sum_roots(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<ArcVarId> {
    let owned_result_non_forwarder = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            matches!(
                c.return_info.uniqueness,
                Uniqueness::Unique | Uniqueness::MaybeShared
            ) && !c.params.iter().any(|p| p.transfers_through_return)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                } => roots.push(*dst),
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if owned_result_non_forwarder(callee) => roots.push(*dst),
                _ => {}
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if owned_result_non_forwarder(callee) {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Gate (c): the root's type is a niche-family sum — variant entries present,
/// no self heap allocation, no struct fields, no element burden, every variant
/// at most ONE owned payload (transfer-on-match binding or retained field).
/// The wrapper then carries no allocation of its own and its RC identity is
/// the single live payload — one release frees the whole web.
pub(in crate::lower::burden_lower) fn is_niche_family_sum(
    func: &ArcFunction,
    root: ArcVarId,
    type_registry: &TypeRegistry,
) -> bool {
    let ty: TypeRef = idx_to_type_ref(func.var_types[root.index()], type_registry);
    let Some(burden) = lookup_burden(ty, type_registry) else {
        return false;
    };
    if burden.self_heap_alloc()
        || burden.owned_fields().next().is_some()
        || burden.element_burden().is_some()
    {
        return false;
    }
    let mut any_variant = false;
    for v in burden.variant_burdens() {
        any_variant = true;
        if v.transfers_on_match.len() + v.retained_owned.len() > 1 {
            return false;
        }
    }
    any_variant
}

/// Gate (d): grow the same-alloc closure from `root` (Let-Var aliases +
/// non-scalar `Project` borrow-views + `Jump`-arg → block-param hops), then
/// vet every member use as a pure borrow-read. `None` on any vet failure.
pub(super) fn same_alloc_closure_vetted(
    func: &ArcFunction,
    root: ArcVarId,
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
                    // Non-scalar Project dsts are the niche payload
                    // borrow-views sharing the allocation (TF-4); scalar tag
                    // reads drop out of the closure.
                    ArcInstr::Project { dst, value, .. }
                        if members.contains(value)
                            && !is_provably_scalar_repr(func, *dst)
                            && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
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

    member_uses_all_borrow_reads(func, &members).then_some(members)
}

/// Gate (d) vetting core: true iff EVERY use of every closure member is a
/// pure borrow-read (no consume / store / capture / COW-machinery use /
/// escape / non-scalar borrowed-call result).
fn member_uses_all_borrow_reads(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches_member = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches_member {
                continue;
            }
            match instr {
                // Alias hops + borrow-view projections are the closure's own
                // edges (scalar tag projections are borrow-reads too).
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. } => {}
                // COW / conditional-alias / mutation / reuse machinery on a
                // member is a distinct sub-root (the Select-branch shape
                // double-frees under a single-release treatment); a closure
                // capture (`PartialApply`) retains a reference — a genuine
                // duplication; an indirect call (`ApplyIndirect`) has no
                // contract to vet. Decline all.
                ArcInstr::Select { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reuse { .. }
                | ArcInstr::CollectionReuse { .. }
                | ArcInstr::PartialApply { .. }
                | ArcInstr::ApplyIndirect { .. } => return false,
                ArcInstr::Apply { dst, .. } => {
                    // Owned-position consume = transfer out of family.
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                    // A borrowed read must provably NOT alias the member into
                    // its result: require a provably-scalar result. The
                    // protocol builtins (`__index` self-inc, `iter` consume)
                    // and user callees returning heap all decline.
                    if !is_provably_scalar_repr(func, *dst) {
                        return false;
                    }
                }
                // Owned-position consume at any other instruction = transfer;
                // a list-concat `PrimOp Binary(Add)` consumes its `RcPointer`
                // operands (the dual-consuming runtime contract) — decline.
                _ => {
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                    if super::super::list_concat_consumed_operands(instr, func)
                        .iter()
                        .any(|v| members.contains(v))
                    {
                        return false;
                    }
                }
            }
        }
        let term = &block.terminator;
        let term_touches = term.used_vars().iter().any(|v| members.contains(v));
        if !term_touches {
            continue;
        }
        match term {
            // Jump hops are the closure's own param edges; Resume/Unreachable
            // use nothing.
            ArcTerminator::Jump { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
            ArcTerminator::Invoke { dst, .. } => {
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return false;
                    }
                }
                // Borrowed terminator read: same provably-scalar-result vet
                // as the body `Apply` arm.
                if !is_provably_scalar_repr(func, *dst) {
                    return false;
                }
            }
            // Escapes / unvettable consumers.
            ArcTerminator::InvokeIndirect { .. }
            | ArcTerminator::Return { .. }
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. } => return false,
        }
    }
    true
}
