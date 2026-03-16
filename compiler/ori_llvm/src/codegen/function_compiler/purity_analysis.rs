//! Purity and readonly analysis for ARC functions.
//!
//! Determines whether a function's ARC IR is free of memory effects
//! (`memory(none)`) or only reads memory (`memory(read)`). Used by
//! [`super::nounwind::compute_nounwind_set`] to assign LLVM memory
//! attributes during the two-pass function compilation.

use ori_arc::ir::{ArcInstr, ArcTerminator};
use ori_ir::Name;

use crate::codegen::abi::{FunctionAbi, ParamPassing, ReturnPassing};

/// Check if an ARC function's IR contains only pure SSA instructions.
///
/// A function qualifies if all its instructions are `Let` (bind
/// constant/variable), `Select` (phi-like), `Construct` (emits LLVM
/// `insertvalue` — pure SSA aggregate creation), or `Project` (emits
/// LLVM `extractvalue` when all params are Direct, but `GEP+load` for
/// Indirect params — reads memory in that case).
///
/// Any call, RC operation, or mutation disqualifies.
///
/// The caller determines the LLVM memory attribute based on the ABI:
/// - `has_only_pure_arc_instructions() && is_abi_memory_free()` → `memory(none)`
///   (all params Direct, so `Project` emits pure `extractvalue`)
/// - `has_only_pure_arc_instructions() && !is_abi_memory_free()` → `memory(read)`
///   (Indirect params cause `Project` to emit `GEP+load`, which reads memory)
pub(super) fn has_only_pure_arc_instructions(func: &ori_arc::ArcFunction) -> bool {
    func.blocks.iter().all(|block| {
        let term_ok = matches!(
            block.terminator,
            ArcTerminator::Return { .. }
                | ArcTerminator::Jump { .. }
                | ArcTerminator::Branch { .. }
                | ArcTerminator::Unreachable
        );
        let instrs_ok = block.body.iter().all(|instr| {
            matches!(
                instr,
                ArcInstr::Let { .. }
                    | ArcInstr::Select { .. }
                    | ArcInstr::Construct { .. }
                    | ArcInstr::Project { .. }
            )
        });
        term_ok && instrs_ok
    })
}

/// Check if a function's ABI has no memory-accessing parameters or return.
///
/// All params must be Direct or Void (no pointer loads), and the return
/// must be Direct or Void (no sret store). Functions with Indirect,
/// Reference, or Sret passing touch memory even if the ARC IR looks pure.
///
/// Used to gate `memory(none)` — a function with pure ARC IR but Indirect
/// params still reads memory via GEP+load for those params.
pub(super) fn is_abi_memory_free(abi: &FunctionAbi) -> bool {
    let params_ok = abi
        .params
        .iter()
        .all(|p| matches!(p.passing, ParamPassing::Direct | ParamPassing::Void));
    let return_ok = matches!(
        abi.return_abi.passing,
        ReturnPassing::Direct | ReturnPassing::Void
    );
    params_ok && return_ok
}

/// Remap `PartialApply { func, .. }` callee names in an ARC function.
///
/// After `declare_and_process_lambda` renames lambdas to globally unique names,
/// the parent function's `PartialApply` instructions still reference the old
/// per-function names (e.g., `__lambda_0`). This function updates them to
/// match the new globally unique names so `emit_partial_apply` finds the
/// correct entry in `codegen_ctx.functions`.
pub(super) fn remap_partial_apply_names(func: &mut ori_arc::ArcFunction, renames: &[(Name, Name)]) {
    for block in &mut func.blocks {
        for instr in &mut block.body {
            if let ArcInstr::PartialApply { ref mut func, .. } = instr {
                for &(old, new) in renames {
                    if *func == old {
                        *func = new;
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
