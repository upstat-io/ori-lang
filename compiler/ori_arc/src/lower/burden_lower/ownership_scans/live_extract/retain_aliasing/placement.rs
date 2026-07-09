//! Gate (f') per-path MULTI-REFERENCE release placement for
//! [`super::compute_retain_aliasing_lineage`]: one per-path release set per
//! retained reference (the niche-sum root + each accessor-retain result),
//! each placed over its own disjoint-live-range sub-closure via the shared
//! MULTI-EXIT placement core.

use rustc_hash::FxHashSet;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::super::super::is_provably_scalar_repr;
use super::super::super::ForwarderReleasePos;

/// Gate (f') per-path MULTI-REFERENCE placement: one per-path release set per
/// retained reference (the niche-sum root for the `@__index` self-inc + each
/// accessor-retain result for its `@unwrap`/`@get` self-inc), each placed over
/// the reference's OWN disjoint-live-range sub-closure
/// ([`retain_aliasing_ref_sub_closure`]) via the shared MULTI-EXIT placement
/// core. Spec: Annex E §AIMS RL-2 + RL-4.
pub(super) fn place_retain_aliasing_per_reference(
    func: &ArcFunction,
    root: ArcVarId,
    members: &FxHashSet<ArcVarId>,
    accessor_retain_names: &FxHashSet<Name>,
) -> Vec<((usize, ForwarderReleasePos), ArcVarId)> {
    let reference_roots: Vec<ArcVarId> = std::iter::once(root)
        .chain(
            members
                .iter()
                .copied()
                .filter(|m| *m != root && accessor_retain_result(func, *m, accessor_retain_names)),
        )
        .collect();
    let mut releases: Vec<((usize, ForwarderReleasePos), ArcVarId)> = Vec::new();
    for ref_root in &reference_roots {
        let sub_closure =
            retain_aliasing_ref_sub_closure(func, *ref_root, members, accessor_retain_names);
        releases.extend(
            super::super::super::multi_exit_borrow_view::place_per_path_releases_with(
                func,
                *ref_root,
                &sub_closure,
                ra_body_read,
                ra_term_read,
            ),
        );
    }
    releases
}

/// The per-reference sub-closure of `ref_root` within the full retain-aliasing
/// `members` closure: `ref_root` plus the `Let { Var }` alias + niche-payload
/// `Project` members reachable from it WITHOUT crossing another
/// accessor-retain hop (each accessor-retain result is its OWN reference with
/// its own sub-closure). This bounds a reference's live range to its own use
/// set: the niche-sum root's sub-closure dies at the accessor-retain call
/// that reads it; an accessor-retain result's sub-closure is born at its call
/// and dies at its last read — releasing each reference exactly once, never
/// on a path where it is unborn/dead.
fn retain_aliasing_ref_sub_closure(
    func: &ArcFunction,
    ref_root: ArcVarId,
    members: &FxHashSet<ArcVarId>,
    accessor_retain_names: &FxHashSet<Name>,
) -> FxHashSet<ArcVarId> {
    let mut sub: FxHashSet<ArcVarId> = FxHashSet::default();
    sub.insert(ref_root);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if sub.contains(src)
                        && members.contains(dst)
                        // Stop at a different reference: an accessor-retain
                        // result is its OWN sub-closure root.
                        && !accessor_retain_result(func, *dst, accessor_retain_names)
                        && sub.insert(*dst) =>
                    {
                        grew = true;
                    }
                    ArcInstr::Project { dst, value, .. }
                        if sub.contains(value)
                            && members.contains(dst)
                            && !is_provably_scalar_repr(func, *dst)
                            && !accessor_retain_result(func, *dst, accessor_retain_names)
                            && sub.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                for (pos, &arg) in args.iter().enumerate() {
                    if sub.contains(&arg) {
                        if let Some(&(param, _)) = func.blocks[target.index()].params.get(pos) {
                            if members.contains(&param)
                                && !accessor_retain_result(func, param, accessor_retain_names)
                                && sub.insert(param)
                            {
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
    sub
}

/// Body final-read predicate for [`super::compute_retain_aliasing_lineage`]'s
/// per-path placement: a member read at a BORROWED (non-owned) `Apply` arg
/// position. The accessor-retain result is itself a member (closure-grown),
/// so a downstream borrow of it (`@starts_with(view [borrow])`) is a member
/// read too.
fn ra_body_read(instr: &ArcInstr, members: &FxHashSet<ArcVarId>) -> Option<ArcVarId> {
    match instr {
        ArcInstr::Apply { .. } => instr.used_vars().iter().enumerate().find_map(|(pos, &v)| {
            (members.contains(&v) && !instr.is_owned_position(pos)).then_some(v)
        }),
        _ => None,
    }
}

/// Terminator analogue of [`ra_body_read`]: a member read at a BORROWED
/// terminator-`Invoke` arg position.
fn ra_term_read(term: &ArcTerminator, members: &FxHashSet<ArcVarId>) -> Option<ArcVarId> {
    match term {
        ArcTerminator::Invoke { .. } => {
            term.used_vars().iter().enumerate().find_map(|(pos, &v)| {
                (members.contains(&v) && !term.is_owned_position(pos)).then_some(v)
            })
        }
        _ => None,
    }
}

/// True iff `var` is the result of an ACCESSOR-RETAIN call (`@unwrap`/`@get` —
/// a builtin in `accessor_retain_names` that self-increments its extracted
/// view). Such a result is a DISTINCT retained reference on the shared
/// allocation (a per-hop self-inc), owing its own per-path release.
pub(super) fn accessor_retain_result(
    func: &ArcFunction,
    var: ArcVarId,
    accessor_retain_names: &FxHashSet<Name>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *dst == var {
                    return accessor_retain_names.contains(callee);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if *dst == var {
                return accessor_retain_names.contains(callee);
            }
        }
    }
    false
}
