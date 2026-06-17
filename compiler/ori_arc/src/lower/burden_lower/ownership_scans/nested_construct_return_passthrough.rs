//! RL-2 in-callee container-passthrough suppression for a fresh nested
//! struct/tuple `Construct` chain whose deepest projection IS the function's
//! Return value, wrapping a transfers-through-return param (annex-e §AIMS RL-2
//! `RL2_release_exactly_once` + TF-3 Construct + TF-4 Project borrow-view).
//!
//! Shape: `@f(xs: [int]) -> [int] = { let i = Inner { items: xs }; let m =
//! Middle { inner: i }; let o = Outer { middle: m }; o.middle.inner.items }`.
//! The param `xs` is moved into a chain of fresh struct/tuple `Construct`s, then
//! projected back out (`Project (Construct args) field == args[field]`) as the
//! Return value. The base walk emits a scope-exit `BurdenDec` on the outermost
//! fresh container; its transitive drop recurses into the deepest field — the
//! transferred-out param — freeing the RETURNED value before the caller reads it
//! (the in-callee use-after-free / double-free, exit -139).
//!
//! The cure: the wrapper chain owns NOTHING that survives the function. The
//! param moves in, then projects back out to the Return; the containers are pure
//! pass-throughs. Suppress the whole construct-chain lineage from
//! `owned_vars_needing_rc` — eliding the surplus container drops that would free
//! the transferred-out param. The param's own transfer-through-return suppresses
//! its callee-side dec; the construct chain's only owned content IS that param,
//! so its drops are surplus releases (proven net-0:
//! `ConstructProjectRoundtrip.wrapper_passthrough_release_exactly_once`).
//!
//! Pairs with the caller-side construct-project round-trip `Direct` return-alias
//! (`interprocedural/extract/alias_flow.rs:resolve_construct_project_roundtrip`),
//! which defers the CALLER's premature param drop; this scan defers the CALLEE's.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};

use crate::lower::burden::Burden;
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

/// RL-2 in-callee container-passthrough suppression. Returns the fresh
/// construct-chain lineage vars to remove from `owned_vars_needing_rc` (eliding
/// the surplus container drops that would free the transferred-out param). Empty
/// when no admitted root.
///
/// Admission gates (ALL hold; ANY failure declines — the status-quo leak/UAF is
/// the migration floor, never a regression introduced here):
///  (a) the function has exactly ONE `Return { value }` whose value resolves,
///      through a chain of `Project (Construct args) field` round-trip hops, to
///      a single PARAM (the construct-project round-trip identity; `EnumVariant`
///      constructs are EXCLUDED — variant-conditional, not a blind round-trip).
///  (b) that param's `ParamContract.transfers_through_return == true` (the param
///      flows out as the return; the caller owns the returned allocation).
///  (c) the wrapper chain (every fresh struct/tuple `Construct` on the round-trip
///      path + its `Let`-Var aliases) has the param as its SOLE owned RC content:
///      each construct's burden has exactly the one owned field that the round-
///      trip projects through. A construct holding a SECOND owned field would
///      leak it under whole-chain suppression — declines.
///  (d) every wrapper-chain member is used ONLY as a construct arg, a `Project`
///      source, or a `Let`-Var alias on the round-trip path (no owned store /
///      capture / second call / second projection escaping the chain).
pub(in crate::lower::burden_lower) fn compute_nested_construct_return_passthrough(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
) -> FxHashSet<ArcVarId> {
    let out = FxHashSet::default();

    // Gate (a) precondition: exactly one Return.
    let returns: Vec<ArcVarId> = func
        .blocks
        .iter()
        .filter_map(|b| match &b.terminator {
            ArcTerminator::Return { value } => Some(*value),
            _ => None,
        })
        .collect();
    let [return_value] = returns.as_slice() else {
        return out;
    };
    let return_value = *return_value;

    // This function's own contract carries the transfers_through_return facts.
    let Some(self_contract) = contracts.get(&func.name) else {
        return out;
    };

    let aliases = build_let_alias_map(func);
    let projects = build_project_map(func);
    let constructs = build_struct_construct_map(func);
    let param_index = build_param_index_map(func);

    // Gate (a): resolve the Return value through the construct-project round-trip
    // to a param, collecting the wrapper-chain construct dsts on the path.
    let mut chain: FxHashSet<ArcVarId> = FxHashSet::default();
    let Some(param_var) = resolve_roundtrip_collecting_chain(
        return_value,
        &aliases,
        &projects,
        &constructs,
        &mut chain,
    ) else {
        return out;
    };
    let Some(&param_idx) = param_index.get(&param_var) else {
        return out;
    };

    // Gate (b): the param transfers through the return (caller owns the result).
    if !self_contract
        .params
        .get(param_idx)
        .is_some_and(|p| p.transfers_through_return)
    {
        return out;
    }

    // The chain must be non-empty (a bare `Return param` with no wrapping is the
    // plain transfers_through_return case, handled elsewhere — no surplus drop).
    if chain.is_empty() {
        return out;
    }

    // Gate (c): each wrapper construct's burden has exactly ONE owned RC field
    // (the round-trip field). A second owned field would leak under suppression.
    for &c in &chain {
        if !construct_sole_owned_rc_field(func, c, type_registry) {
            return out;
        }
    }

    // Gate (d): every chain member (+ its Let-Var aliases) is used ONLY on the
    // round-trip path (construct arg / Project source / Let-Var alias). Any other
    // use (owned store, capture, second projection, call arg) declines.
    let mut chain_with_aliases = chain.clone();
    grow_let_aliases(&mut chain_with_aliases, &aliases);
    if !chain_members_only_roundtrip_used(func, &chain_with_aliases, return_value) {
        return out;
    }

    // Admitted: suppress the whole wrapper-chain lineage (constructs + their
    // Let-Var aliases) from owned_vars_needing_rc. Restrict to vars actually
    // tracked (scalars / borrowed already filtered upstream).
    chain_with_aliases
        .into_iter()
        .filter(|v| owned_vars_needing_rc.contains(v))
        .collect()
}

/// Resolve `var` through the construct-project round-trip to its param leaf,
/// collecting every struct/tuple `Construct` dst traversed into `chain`. Mirrors
/// `interprocedural/extract/alias_flow.rs:resolve_value` (the caller-side twin)
/// but additionally records the wrapper-chain constructs for suppression.
fn resolve_roundtrip_collecting_chain(
    var: ArcVarId,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
    projects: &FxHashMap<ArcVarId, (ArcVarId, u32)>,
    constructs: &FxHashMap<ArcVarId, Vec<ArcVarId>>,
    chain: &mut FxHashSet<ArcVarId>,
) -> Option<ArcVarId> {
    let mut visited: FxHashSet<ArcVarId> = FxHashSet::default();
    resolve_roundtrip_rec(var, aliases, projects, constructs, chain, &mut visited)
}

fn resolve_roundtrip_rec(
    var: ArcVarId,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
    projects: &FxHashMap<ArcVarId, (ArcVarId, u32)>,
    constructs: &FxHashMap<ArcVarId, Vec<ArcVarId>>,
    chain: &mut FxHashSet<ArcVarId>,
    visited: &mut FxHashSet<ArcVarId>,
) -> Option<ArcVarId> {
    let leaf = resolve_let_leaf(var, aliases);
    if !visited.insert(leaf) {
        return None; // cycle guard
    }
    if let Some(&(proj_src, field)) = projects.get(&leaf) {
        let src_val =
            resolve_roundtrip_rec(proj_src, aliases, projects, constructs, chain, visited)?;
        if let Some(args) = constructs.get(&src_val) {
            chain.insert(src_val);
            let arg = *args.get(field as usize)?;
            return resolve_roundtrip_rec(arg, aliases, projects, constructs, chain, visited);
        }
        // Project of a non-construct source — the value is the leaf itself (not a
        // round-trip; an opaque borrow-view). Not a param-passthrough chain.
        return Some(leaf);
    }
    Some(leaf)
}

/// `Let { dst, Var(src) }` alias map (dst -> src).
fn build_let_alias_map(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut out = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                out.insert(*dst, *src);
            }
        }
    }
    out
}

/// `Project { dst, value, field }` map (dst -> (src, field)).
fn build_project_map(func: &ArcFunction) -> FxHashMap<ArcVarId, (ArcVarId, u32)> {
    let mut out = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project {
                dst, value, field, ..
            } = instr
            {
                out.insert(*dst, (*value, *field));
            }
        }
    }
    out
}

/// Struct/tuple `Construct` map (dst -> field args). `EnumVariant` EXCLUDED.
fn build_struct_construct_map(func: &ArcFunction) -> FxHashMap<ArcVarId, Vec<ArcVarId>> {
    let mut out = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct {
                dst, ctor, args, ..
            } = instr
            {
                if matches!(ctor, CtorKind::Struct(_) | CtorKind::Tuple) {
                    out.insert(*dst, args.clone());
                }
            }
        }
    }
    out
}

/// Param var -> param index map.
fn build_param_index_map(func: &ArcFunction) -> FxHashMap<ArcVarId, usize> {
    func.params
        .iter()
        .enumerate()
        .map(|(i, p)| (p.var, i))
        .collect()
}

/// Resolve `var` through `Let`-Var alias edges to its leaf.
fn resolve_let_leaf(var: ArcVarId, aliases: &FxHashMap<ArcVarId, ArcVarId>) -> ArcVarId {
    let mut cur = var;
    let mut visited: FxHashSet<ArcVarId> = FxHashSet::default();
    while visited.insert(cur) {
        match aliases.get(&cur) {
            Some(&src) => cur = src,
            None => break,
        }
    }
    cur
}

/// Grow `set` with every `Let`-Var alias (both directions) of its members.
fn grow_let_aliases(set: &mut FxHashSet<ArcVarId>, aliases: &FxHashMap<ArcVarId, ArcVarId>) {
    loop {
        let mut grew = false;
        for (&dst, &src) in aliases {
            if set.contains(&src) && set.insert(dst) {
                grew = true;
            }
            if set.contains(&dst) && set.insert(src) {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
}

/// Gate (c): the construct `c`'s monomorphized burden has EXACTLY ONE owned RC
/// field (the round-trip field the wrapper passes through). A multi-owned-field
/// construct would leak its other fields under whole-chain suppression.
fn construct_sole_owned_rc_field(
    func: &ArcFunction,
    c: ArcVarId,
    type_registry: &TypeRegistry,
) -> bool {
    let ty = idx_to_type_ref(func.var_types[c.index()], type_registry);
    let Some(burden) = lookup_burden(ty, type_registry) else {
        return false;
    };
    burden.owned_fields().count() == 1
}

/// Gate (d): every chain member is used ONLY on the round-trip path — as a
/// `Construct` arg, a `Project` source, or a `Let`-Var alias. The Return value
/// itself is exempt (it IS the round-trip terminus). Any other use (owned store,
/// capture, `Apply`/`Invoke`/`Set` operand, `Branch`/`Switch`/`Jump`) declines.
fn chain_members_only_roundtrip_used(
    func: &ArcFunction,
    chain: &FxHashSet<ArcVarId>,
    return_value: ArcVarId,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                // Construct arg / Project source / Let-Var alias are the chain's
                // own round-trip edges — permitted uses of a chain member.
                ArcInstr::Construct { .. }
                | ArcInstr::Project { .. }
                | ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                } => {}
                other => {
                    if other.used_vars().iter().any(|v| chain.contains(v)) {
                        return false;
                    }
                }
            }
        }
        // Terminator: only the single `Return return_value` may name a chain
        // member (the round-trip terminus). Any other terminator use declines.
        match &block.terminator {
            ArcTerminator::Return { value } if *value == return_value => {}
            term => {
                if term.used_vars().iter().any(|v| chain.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}
