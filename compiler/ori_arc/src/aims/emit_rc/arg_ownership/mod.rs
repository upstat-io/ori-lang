//! Argument ownership annotation for AIMS call-event realization.
//!
//! Populates `arg_ownership` on `Apply`/`Invoke`/`ApplyIndirect`/
//! `InvokeIndirect` instructions from [`MemoryContract`] signatures and
//! exact callable identities.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::{Name, StringInterner};
use ori_types::Pool;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::{AccessClass, Cardinality, Consumption};
use crate::ir::ArcFunction;
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::BuiltinOwnershipSets;

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

fn contract_signature(contract: &MemoryContract) -> AnnotatedSig {
    AnnotatedSig {
        params: contract_to_params(contract),
        return_type: ori_types::Idx::NONE,
    }
}

fn add_original_name_signatures(
    signatures: &mut FxHashMap<Name, AnnotatedSig>,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
) {
    let entries: Vec<(Name, AnnotatedSig)> = contracts
        .iter()
        .filter_map(|(&name, contract)| {
            let name_str = interner.try_lookup(name)?;
            let separator = name_str.find(ori_ir::MONO_SEPARATOR)?;
            let original_name = interner.intern(&name_str[..separator]);
            if signatures.contains_key(&original_name) {
                return None;
            }
            Some((original_name, contract_signature(contract)))
        })
        .collect();

    for (original_name, signature) in entries {
        signatures
            .entry(original_name)
            .and_modify(|existing| {
                for (index, param) in signature.params.iter().enumerate() {
                    if param.ownership == Ownership::Borrowed {
                        if let Some(existing_param) = existing.params.get_mut(index) {
                            existing_param.ownership = Ownership::Borrowed;
                        }
                    }
                }
            })
            .or_insert(signature);
    }
}

/// Gives exact callables precedence over same-spelled builtin ownership rules.
pub(crate) fn emit_arg_ownership(
    func: &mut ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
    builtins: &BuiltinOwnershipSets,
    pool: &Pool,
    exact_callables: &FxHashSet<Name>,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let mut sigs: FxHashMap<Name, AnnotatedSig> = contracts
        .iter()
        .map(|(&name, contract)| (name, contract_signature(contract)))
        .collect();

    add_original_name_signatures(&mut sigs, contracts, interner);

    crate::rc_insert::annotate_arg_ownership(
        func,
        &sigs,
        interner,
        builtins,
        pool,
        exact_callables,
    );

    let errors = crate::verify::check_total_arg_ownership(func);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests;
