//! Return-contract extraction: structural definition tracing for
//! return-value uniqueness and freshness.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::super::super::contract::{MemoryContract, ReturnContract};
use super::super::super::lattice::Uniqueness;

use super::{build_definition_map, build_invoke_def_map};

// Return info extraction

/// Determine return value uniqueness from the function's Return terminators.
///
/// Walks all Return terminators, traces each returned variable to its
/// definition instruction, and determines uniqueness based on how the
/// value was produced. Results from all return paths are joined.
pub(super) fn extract_return_info(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> ReturnContract {
    // Build definition map: var → defining instruction.
    let def_map = build_definition_map(func);

    // Build Invoke definition map: dst → callee name
    // (Invoke is a terminator, not an instruction).
    let invoke_defs = build_invoke_def_map(func);

    // Collect parameter variables for identity checks.
    let param_vars: rustc_hash::FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();

    let mut return_uniqueness = None::<Uniqueness>;
    let mut all_preserve_freshness = true;

    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            let (uniq, preserves) = var_uniqueness(
                *value,
                &def_map,
                &invoke_defs,
                &param_vars,
                classifier,
                sigs,
            );

            return_uniqueness = Some(match return_uniqueness {
                None => uniq,
                Some(prev) => prev.join(uniq),
            });

            if !preserves {
                all_preserve_freshness = false;
            }
        }
    }

    match return_uniqueness {
        Some(uniqueness) => ReturnContract {
            uniqueness,
            preserves_freshness: all_preserve_freshness,
            ..ReturnContract::CONSERVATIVE
        },
        // No Return terminators (e.g., infinite loop) — conservative.
        None => ReturnContract::CONSERVATIVE,
    }
}

/// Determine the uniqueness of a variable based on its definition.
///
/// Returns `(Uniqueness, preserves_freshness)`.
fn var_uniqueness(
    var: ArcVarId,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    invoke_defs: &FxHashMap<ArcVarId, Name>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(instr) = def_map.get(&var) {
        match instr {
            // Fresh construction, closure, COW reuse, or scalar check → unique.
            ArcInstr::Construct { .. }
            | ArcInstr::Reuse { .. }
            | ArcInstr::PartialApply { .. }
            | ArcInstr::CollectionReuse { .. }
            | ArcInstr::IsShared { .. } => (Uniqueness::Unique, true),

            // Direct call → use callee's return contract.
            ArcInstr::Apply { func: callee, .. } => callee_return_uniqueness(*callee, sigs),

            // Let binding → trace through to source.
            ArcInstr::Let { value, ty, .. } => match value {
                crate::ir::ArcValue::Var(source) => {
                    if param_vars.contains(source) {
                        // Returning a parameter → preserves freshness
                        // (if caller passes unique, return is unique).
                        (Uniqueness::MaybeShared, true)
                    } else {
                        var_uniqueness(*source, def_map, invoke_defs, param_vars, classifier, sigs)
                    }
                }
                crate::ir::ArcValue::Literal(_) => (Uniqueness::Unique, true),
                crate::ir::ArcValue::PrimOp { .. } => {
                    if classifier.is_scalar(*ty) {
                        (Uniqueness::Unique, true)
                    } else {
                        (Uniqueness::MaybeShared, false)
                    }
                }
            },

            // Indirect call, projection, select, RC/mutation ops → conservative.
            // BurdenInc/BurdenDec are side-effect-only annotations (no dst);
            // they cannot appear in def_map but the exhaustive match must
            // cover them — group with the other side-effect-only ops.
            ArcInstr::ApplyIndirect { .. }
            | ArcInstr::Project { .. }
            | ArcInstr::Select { .. }
            | ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::RcDecPartial { .. }
            | ArcInstr::RcDecField { .. }
            | ArcInstr::RcDecVariant { .. }
            | ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::BurdenDecPartial { .. }
            | ArcInstr::BurdenDecField { .. }
            | ArcInstr::BurdenDecVariant { .. }
            | ArcInstr::Set { .. }
            | ArcInstr::SetTag { .. }
            | ArcInstr::Reset { .. } => (Uniqueness::MaybeShared, false),
        }
    } else if let Some(callee) = invoke_defs.get(&var) {
        // Defined by an Invoke terminator → use callee's return contract.
        callee_return_uniqueness(*callee, sigs)
    } else if param_vars.contains(&var) {
        // Returning a parameter directly → uniqueness depends on caller.
        (Uniqueness::MaybeShared, true)
    } else {
        // Block parameter or unknown definition → conservative.
        (Uniqueness::MaybeShared, false)
    }
}

/// Look up a callee's return uniqueness from its contract.
fn callee_return_uniqueness(
    callee: Name,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(contract) = sigs.get(&callee) {
        (
            contract.return_info.uniqueness,
            contract.return_info.preserves_freshness,
        )
    } else {
        (Uniqueness::MaybeShared, false)
    }
}
