//! Empty trampoline-block elimination.

use inkwell::values::FunctionValue;
use llvm_sys::core;
use llvm_sys::prelude::LLVMBasicBlockRef;

use super::{
    build_pred_map, has_phi_nodes, is_single_unconditional_br, redirect_terminator, rewrite_phis,
    would_create_duplicate_phi,
};

/// Eliminate one empty block and report whether the pass changed the CFG.
pub(super) fn eliminate_empty_blocks(function: FunctionValue<'_>) -> u32 {
    let blocks = function.get_basic_blocks();
    let entry_ref = match blocks.first() {
        Some(block) => block.as_mut_ptr(),
        None => return 0,
    };
    let pred_map = build_pred_map(function);
    let mut candidates: Vec<(inkwell::basic_block::BasicBlock<'_>, LLVMBasicBlockRef)> = Vec::new();

    for block in &blocks {
        let block_ref = block.as_mut_ptr();
        if block_ref == entry_ref || has_phi_nodes(block_ref) || !is_single_unconditional_br(block)
        {
            continue;
        }
        let target = unsafe {
            let term = core::LLVMGetBasicBlockTerminator(block_ref);
            core::LLVMGetSuccessor(term, 0)
        };
        if target != block_ref {
            candidates.push((*block, target));
        }
    }

    for (empty_block, target_ref) in &candidates {
        let empty_ref = empty_block.as_mut_ptr();
        let preds = pred_map.get(&empty_ref).cloned().unwrap_or_else(|| {
            unreachable!("build_pred_map pre-populates every block of the function")
        });
        if would_create_duplicate_phi(*target_ref, empty_ref, &preds) {
            continue;
        }
        rewrite_phis(*target_ref, empty_ref, &preds);
        for &pred_ref in &preds {
            redirect_terminator(pred_ref, empty_ref, *target_ref);
        }
        unsafe {
            let term = core::LLVMGetBasicBlockTerminator(empty_ref);
            if !term.is_null() {
                core::LLVMInstructionEraseFromParent(term);
            }
            core::LLVMRemoveBasicBlockFromParent(empty_ref);
        }
        return 1;
    }
    0
}
