//! Argument ownership annotation for AIMS RC emission.
//!
//! Populates `arg_ownership` on `Apply`/`Invoke`/`ApplyIndirect`/
//! `InvokeIndirect` instructions from [`MemoryContract`] signatures.
//! Delegates to the existing
//! [`annotate_arg_ownership`](crate::rc_insert::annotate_arg_ownership)
//! after converting contracts to [`AnnotatedSig`]s.
//!
//! # Current implementation
//!
//! Currently a thin wrapper: converts `MemoryContract` → `AnnotatedSig`
//! via contract fields, then calls the existing annotation function.
//! This avoids duplicating the type-qualified builtin dispatch logic
//! (250+ lines). Future versions should replace this with direct
//! `MemoryContract` consumption.

use rustc_hash::FxHashMap;

use ori_ir::{Name, StringInterner};
use ori_types::Pool;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::{AccessClass, Cardinality, Consumption};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator};
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::BuiltinOwnershipSets;

/// Convert a `MemoryContract`'s params to `AnnotatedParam` list.
fn contract_to_params(contract: &MemoryContract) -> Vec<AnnotatedParam> {
    contract
        .params
        .iter()
        .enumerate()
        .map(|(i, pc)| {
            let ownership =
                if pc.consumption == Consumption::Dead || pc.cardinality == Cardinality::Absent {
                    Ownership::Borrowed
                } else {
                    match pc.access {
                        AccessClass::Borrowed => Ownership::Borrowed,
                        AccessClass::Owned => Ownership::Owned,
                    }
                };
            AnnotatedParam {
                name: Name::from_raw(
                    u32::try_from(i).unwrap_or_else(|_| panic!("param index {i} exceeds u32::MAX")),
                ),
                ty: ori_types::Idx::NONE,
                ownership,
            }
        })
        .collect()
}

/// Populate `arg_ownership` on all call sites from AIMS contracts.
///
/// Converts each `MemoryContract` to an `AnnotatedSig` and delegates to
/// [`crate::rc_insert::annotate_arg_ownership`]. This preserves the
/// type-qualified builtin dispatch logic.
///
/// Must be called **before** RC emission (pipeline step 4 in Section 06.2).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn emit_arg_ownership(
    func: &mut ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
    builtins: &BuiltinOwnershipSets,
    pool: &Pool,
) {
    let mut sigs: FxHashMap<Name, AnnotatedSig> = contracts
        .iter()
        .map(|(&name, contract)| {
            let params = contract_to_params(contract);
            (
                name,
                AnnotatedSig {
                    params,
                    return_type: ori_types::Idx::NONE,
                },
            )
        })
        .collect();

    // Monomorphized name resolution: ARC IR call sites use the original
    // function name (e.g., "apply"), but contracts are keyed under the
    // monomorphized name (e.g., "apply$m$Lint"). Add entries under the
    // original name so call sites find the correct ownership annotations.
    // When multiple monomorphizations exist, use conservative merge
    // (Borrowed if ANY mono says Borrowed — safe: caller retains cleanup).
    let mono_entries: Vec<(Name, AnnotatedSig)> = contracts
        .iter()
        .filter_map(|(&name, contract)| {
            let name_str = interner.try_lookup(name)?;
            let pos = name_str.find(ori_ir::MONO_SEPARATOR)?;
            let orig_name = interner.intern(&name_str[..pos]);
            // Skip if original name already in sigs (non-monomorphized version exists)
            if sigs.contains_key(&orig_name) {
                return None;
            }
            Some((
                orig_name,
                AnnotatedSig {
                    params: contract_to_params(contract),
                    return_type: ori_types::Idx::NONE,
                },
            ))
        })
        .collect();

    for (orig_name, sig) in mono_entries {
        sigs.entry(orig_name)
            .and_modify(|existing| {
                // Conservative merge: Borrowed wins (safer — caller retains cleanup)
                for (i, param) in sig.params.iter().enumerate() {
                    if let Some(ep) = existing.params.get_mut(i) {
                        if param.ownership == Ownership::Borrowed {
                            ep.ownership = Ownership::Borrowed;
                        }
                    }
                }
            })
            .or_insert(sig);
    }

    crate::rc_insert::annotate_arg_ownership(func, &sigs, interner, builtins, pool);

    // Invariant: after annotation, every indirect call with non-empty args
    // must have non-empty arg_ownership. Empty arg_ownership on a call with
    // args means annotation missed it — a bug.
    debug_assert!(
        {
            let mut ok = true;
            for block in &func.blocks {
                for instr in &block.body {
                    if let ArcInstr::ApplyIndirect {
                        args,
                        arg_ownership,
                        ..
                    } = instr
                    {
                        if !args.is_empty() && arg_ownership.is_empty() {
                            ok = false;
                        }
                    }
                }
                if let ArcTerminator::InvokeIndirect {
                    args,
                    arg_ownership,
                    ..
                } = &block.terminator
                {
                    if !args.is_empty() && arg_ownership.is_empty() {
                        ok = false;
                    }
                }
            }
            ok
        },
        "emit_arg_ownership: indirect call with non-empty args has empty arg_ownership"
    );
}

#[cfg(test)]
mod tests;
