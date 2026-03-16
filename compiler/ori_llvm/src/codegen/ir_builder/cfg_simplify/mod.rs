//! Post-emission CFG simplification pass.
//!
//! Eliminates empty blocks and redundant branches in LLVM IR after all
//! instructions for a function have been emitted. Runs before LLVM
//! verification.
//!
//! # Patterns Handled
//!
//! 1. **Empty blocks**: A block with no phi nodes and only an unconditional
//!    `br label %target` is eliminated. Predecessors are redirected to the
//!    target, and phi incoming entries are rewritten by rebuilding the phi.
//!    Blocks that are structurally necessary (removing them would create
//!    duplicate phi entries from the same predecessor) are preserved.
//!
//! 2. **Redundant conditional branches**: `br i1 %cond, label %X, label %X`
//!    where both arms target the same block is replaced with `br label %X`.
//!
//! Entry blocks are never removed (LLVM requires the first block to remain).
//! Chained empty blocks are handled by iterating to fixed point.
//!
//! # Performance
//!
//! The predecessor map is built once per pass iteration. Each empty block
//! removal is O(predecessors + phi entries). Fixed-point loop terminates
//! when no blocks are removed — guaranteed since each pass removes ≥1
//! block or terminates.
//!
//! # LLVM C API limitation
//!
//! The C API has no `LLVMSetIncomingBlock` — only getters and `LLVMAddIncoming`.
//! Phi rewriting therefore rebuilds each affected phi: collect (value, block)
//! pairs with the old block replaced, create a new phi, RAUW, delete old.

use std::collections::HashMap;

use inkwell::values::{AsValueRef, FunctionValue};
use llvm_sys::core;
use llvm_sys::prelude::LLVMBasicBlockRef;

/// Statistics from the CFG simplification pass.
#[derive(Debug, Default)]
pub struct SimplifyStats {
    /// Number of empty blocks removed.
    pub blocks_removed: u32,
    /// Number of redundant conditional branches simplified.
    pub branches_simplified: u32,
}

/// Simplify the CFG of a function by eliminating empty blocks and redundant
/// branches.
///
/// Run AFTER all IR is emitted, BEFORE function verification. Iterates to
/// fixed point to handle chained empty blocks.
pub fn simplify_cfg(function: FunctionValue<'_>) -> SimplifyStats {
    let mut stats = SimplifyStats::default();

    simplify_redundant_branches(function, &mut stats);

    loop {
        let removed = eliminate_empty_blocks(function);
        if removed == 0 {
            break;
        }
        stats.blocks_removed += removed;
    }

    if merge_entry_block(function) {
        stats.blocks_removed += 1;
    }

    stats
}

/// Replace `br i1 %c, label %X, label %X` with `br label %X`.
fn simplify_redundant_branches(function: FunctionValue<'_>, stats: &mut SimplifyStats) {
    for block in function.get_basic_blocks() {
        let Some(term) = block.get_terminator() else {
            continue;
        };
        let term_ref = term.as_value_ref();

        if unsafe { core::LLVMGetInstructionOpcode(term_ref) } != llvm_sys::LLVMOpcode::LLVMBr {
            continue;
        }
        if unsafe { core::LLVMGetNumSuccessors(term_ref) } != 2 {
            continue;
        }
        let succ0 = unsafe { core::LLVMGetSuccessor(term_ref, 0) };
        let succ1 = unsafe { core::LLVMGetSuccessor(term_ref, 1) };
        if succ0 != succ1 {
            continue;
        }

        // SAFETY: erasing a valid terminator and building a replacement.
        unsafe {
            core::LLVMInstructionEraseFromParent(term_ref);
        }
        let target = unsafe { inkwell::basic_block::BasicBlock::new(succ0).unwrap() };
        let ctx = function.get_type().get_context();
        let tmp_builder = ctx.create_builder();
        tmp_builder.position_at_end(block);
        tmp_builder.build_unconditional_branch(target).unwrap();

        stats.branches_simplified += 1;
    }
}

/// Build predecessor map: block -> list of unique predecessor blocks.
fn build_pred_map(
    function: FunctionValue<'_>,
) -> HashMap<LLVMBasicBlockRef, Vec<LLVMBasicBlockRef>> {
    let mut map: HashMap<LLVMBasicBlockRef, Vec<LLVMBasicBlockRef>> = HashMap::new();
    for block in function.get_basic_blocks() {
        let block_ref = block.as_mut_ptr();
        map.entry(block_ref).or_default();
        let Some(term) = block.get_terminator() else {
            continue;
        };
        let term_ref = term.as_value_ref();
        let n = unsafe { core::LLVMGetNumSuccessors(term_ref) };
        for i in 0..n {
            let succ = unsafe { core::LLVMGetSuccessor(term_ref, i) };
            let preds = map.entry(succ).or_default();
            if !preds.contains(&block_ref) {
                preds.push(block_ref);
            }
        }
    }
    map
}

/// Eliminate empty blocks. Returns the number removed in this pass.
fn eliminate_empty_blocks(function: FunctionValue<'_>) -> u32 {
    let blocks = function.get_basic_blocks();
    let entry_ref = match blocks.first() {
        Some(b) => b.as_mut_ptr(),
        None => return 0,
    };

    let pred_map = build_pred_map(function);

    // Collect candidates: (block, target)
    let mut candidates: Vec<(inkwell::basic_block::BasicBlock<'_>, LLVMBasicBlockRef)> = Vec::new();

    for block in &blocks {
        let block_ref = block.as_mut_ptr();

        if block_ref == entry_ref {
            continue;
        }
        if has_phi_nodes(block_ref) {
            continue;
        }
        if !is_single_unconditional_br(block) {
            continue;
        }

        let target = unsafe {
            let term = core::LLVMGetBasicBlockTerminator(block_ref);
            core::LLVMGetSuccessor(term, 0)
        };
        candidates.push((*block, target));
    }

    // Process ONE candidate per pass to avoid stale pred_map issues.
    // The fixed-point loop in simplify_cfg handles subsequent candidates
    // by rebuilding the pred_map each iteration.
    for (empty_block, target_ref) in &candidates {
        let empty_ref = empty_block.as_mut_ptr();
        let preds = pred_map.get(&empty_ref).cloned().unwrap_or_default();

        if would_create_duplicate_phi(*target_ref, empty_ref, &preds) {
            continue;
        }

        // Step 1: Rewrite phi incoming entries in the target block.
        rewrite_phis(*target_ref, empty_ref, &preds);

        // Step 2: Redirect predecessors' terminators to jump to target.
        for &pred_ref in &preds {
            redirect_terminator(pred_ref, empty_ref, *target_ref);
        }

        // Step 3: Remove the empty block.
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

/// Merge the entry block with its sole successor when:
///
/// 1. Entry block contains only `br label %header`
/// 2. Header has exactly one predecessor (the entry block)
///
/// Instead of moving instructions, we swap block positions: move header
/// before entry (making it the new entry), then delete the old entry.
/// Loop headers with back-edges (>1 predecessor) are left alone — these
/// are structurally necessary preheaders.
fn merge_entry_block(function: FunctionValue<'_>) -> bool {
    let blocks = function.get_basic_blocks();
    if blocks.len() < 2 {
        return false;
    }
    let entry = &blocks[0];
    if !is_single_unconditional_br(entry) {
        return false;
    }

    let entry_ref = entry.as_mut_ptr();
    let header_ref = unsafe {
        let term = core::LLVMGetBasicBlockTerminator(entry_ref);
        core::LLVMGetSuccessor(term, 0)
    };

    // Check header has exactly one predecessor (the entry block).
    let pred_map = build_pred_map(function);
    let preds = pred_map.get(&header_ref).cloned().unwrap_or_default();
    if preds.len() != 1 || preds[0] != entry_ref {
        return false;
    }

    // SAFETY: entry has exactly one instruction (br label %header) and
    // header has exactly one predecessor (entry). No phi nodes in header
    // (single predecessor). Moving header before entry makes it the new
    // entry point (LLVM entry = first block in function).
    unsafe {
        // Move header before entry → header becomes the first block.
        core::LLVMMoveBasicBlockBefore(header_ref, entry_ref);

        // Delete old entry's terminator, then remove the block.
        let entry_term = core::LLVMGetBasicBlockTerminator(entry_ref);
        if !entry_term.is_null() {
            core::LLVMInstructionEraseFromParent(entry_term);
        }
        core::LLVMRemoveBasicBlockFromParent(entry_ref);
    }
    true
}

/// Check if a block is a single unconditional br (the only instruction).
fn is_single_unconditional_br(block: &inkwell::basic_block::BasicBlock<'_>) -> bool {
    let Some(term) = block.get_terminator() else {
        return false;
    };
    let first = block.get_first_instruction();
    if first.as_ref().map(AsValueRef::as_value_ref) != Some(term.as_value_ref()) {
        return false;
    }
    let term_ref = term.as_value_ref();
    let is_br = unsafe { core::LLVMGetInstructionOpcode(term_ref) } == llvm_sys::LLVMOpcode::LLVMBr;
    let is_uncond = unsafe { core::LLVMGetNumSuccessors(term_ref) } == 1;
    is_br && is_uncond
}

/// Check if a block has any phi nodes.
fn has_phi_nodes(block: LLVMBasicBlockRef) -> bool {
    let first = unsafe { core::LLVMGetFirstInstruction(block) };
    if first.is_null() {
        return false;
    }
    unsafe { core::LLVMGetInstructionOpcode(first) == llvm_sys::LLVMOpcode::LLVMPHI }
}

/// Check if removing `old_block` from target's phi nodes (replacing with
/// its predecessors) would create duplicate incoming entries from the same
/// basic block with different values.
fn would_create_duplicate_phi(
    target: LLVMBasicBlockRef,
    old_block: LLVMBasicBlockRef,
    predecessors: &[LLVMBasicBlockRef],
) -> bool {
    let mut inst = unsafe { core::LLVMGetFirstInstruction(target) };
    while !inst.is_null() {
        if unsafe { core::LLVMGetInstructionOpcode(inst) } != llvm_sys::LLVMOpcode::LLVMPHI {
            break;
        }
        let num = unsafe { core::LLVMCountIncoming(inst) };

        // Collect all blocks that would be in the phi after rewrite.
        // If any predecessor already appears, it's a duplicate.
        for &pred in predecessors {
            for i in 0..num {
                let bb = unsafe { core::LLVMGetIncomingBlock(inst, i) };
                if bb != old_block && bb == pred {
                    return true;
                }
            }
        }

        inst = unsafe { core::LLVMGetNextInstruction(inst) };
    }
    false
}

/// Rewrite phi nodes in `target`: replace incoming entries from `old_block`
/// with entries from its predecessors.
///
/// Since LLVM's C API has no `LLVMSetIncomingBlock`, we rebuild each
/// affected phi: collect (value, block) pairs with `old_block` entries
/// replaced by predecessor entries, build a new phi, RAUW, delete old.
fn rewrite_phis(
    target: LLVMBasicBlockRef,
    old_block: LLVMBasicBlockRef,
    predecessors: &[LLVMBasicBlockRef],
) {
    if predecessors.is_empty() {
        return;
    }

    let mut inst = unsafe { core::LLVMGetFirstInstruction(target) };
    while !inst.is_null() {
        if unsafe { core::LLVMGetInstructionOpcode(inst) } != llvm_sys::LLVMOpcode::LLVMPHI {
            break;
        }
        let next = unsafe { core::LLVMGetNextInstruction(inst) };

        let num = unsafe { core::LLVMCountIncoming(inst) };
        let has_old = (0..num).any(|i| unsafe { core::LLVMGetIncomingBlock(inst, i) } == old_block);
        if !has_old {
            inst = next;
            continue;
        }

        // Collect new incoming pairs.
        let mut values = Vec::with_capacity(num as usize + predecessors.len());
        let mut blocks = Vec::with_capacity(num as usize + predecessors.len());
        for i in 0..num {
            let bb = unsafe { core::LLVMGetIncomingBlock(inst, i) };
            let val = unsafe { core::LLVMGetIncomingValue(inst, i) };
            if bb == old_block {
                // Replace with one entry per predecessor (same value).
                for &pred in predecessors {
                    values.push(val);
                    blocks.push(pred);
                }
            } else {
                values.push(val);
                blocks.push(bb);
            }
        }

        // Rebuild: new phi → add incoming → RAUW → delete old.
        // SAFETY: inst is a valid phi, we create a replacement at the same
        // position, transfer all uses, then delete the original.
        unsafe {
            let phi_ty = core::LLVMTypeOf(inst);
            let ctx = core::LLVMGetTypeContext(phi_ty);
            let builder = core::LLVMCreateBuilderInContext(ctx);
            core::LLVMPositionBuilderBefore(builder, inst);

            let mut name_len = 0;
            let name = core::LLVMGetValueName2(inst, std::ptr::addr_of_mut!(name_len));
            let new_phi = core::LLVMBuildPhi(builder, phi_ty, name);
            core::LLVMAddIncoming(
                new_phi,
                values.as_mut_ptr(),
                blocks.as_mut_ptr(),
                values.len() as u32,
            );
            core::LLVMReplaceAllUsesWith(inst, new_phi);
            core::LLVMInstructionEraseFromParent(inst);
            core::LLVMDisposeBuilder(builder);
        }

        inst = next;
    }
}

/// Redirect a block's terminator: replace successor `old_target` with `new_target`.
fn redirect_terminator(
    block: LLVMBasicBlockRef,
    old_target: LLVMBasicBlockRef,
    new_target: LLVMBasicBlockRef,
) {
    let term = unsafe { core::LLVMGetBasicBlockTerminator(block) };
    if term.is_null() {
        return;
    }
    let n = unsafe { core::LLVMGetNumSuccessors(term) };
    for i in 0..n {
        if unsafe { core::LLVMGetSuccessor(term, i) } == old_target {
            unsafe {
                core::LLVMSetSuccessor(term, i, new_target);
            }
        }
    }
}

#[cfg(test)]
mod tests;
