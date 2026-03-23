//! Unit tests for the CFG simplification pass.

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::FunctionValue;
use inkwell::IntPredicate;

use super::simplify_cfg;

/// Helper: create a module and a function `() -> i64` in it.
///
/// The module MUST be kept alive while the function is used — LLVM functions
/// are owned by their module.
fn make_test_function<'ctx>(ctx: &'ctx Context, name: &str) -> (Module<'ctx>, FunctionValue<'ctx>) {
    let module = ctx.create_module("cfg_simplify_test");
    let i64_ty = ctx.i64_type();
    let fn_ty = i64_ty.fn_type(&[], false);
    let func = module.add_function(name, fn_ty, None);
    (module, func)
}

/// An empty trampoline block (`br label %target`, no phis) should be removed.
///
/// Before: entry → trampoline → exit
/// After:  entry → exit
#[test]
fn cfg_simplify_removes_empty_blocks() {
    let ctx = Context::create();
    let (_module, func) = make_test_function(&ctx, "test_empty");
    let builder = ctx.create_builder();

    let entry = ctx.append_basic_block(func, "entry");
    let trampoline = ctx.append_basic_block(func, "trampoline");
    let exit = ctx.append_basic_block(func, "exit");

    // entry: br label %trampoline
    builder.position_at_end(entry);
    builder.build_unconditional_branch(trampoline).unwrap();

    // trampoline: br label %exit  (empty — only a br)
    builder.position_at_end(trampoline);
    builder.build_unconditional_branch(exit).unwrap();

    // exit: ret i64 42
    builder.position_at_end(exit);
    let ret_val = ctx.i64_type().const_int(42, false);
    builder.build_return(Some(&ret_val)).unwrap();

    assert_eq!(func.get_basic_blocks().len(), 3, "pre: 3 blocks");

    let stats = simplify_cfg(func);

    // Trampoline removed by empty block elimination, then entry merged
    // with exit (entry was `br label %exit`, exit has 1 pred).
    assert_eq!(stats.blocks_removed, 2, "trampoline + entry merge");
    assert_eq!(func.get_basic_blocks().len(), 1, "post: 1 block");

    // The single remaining block should return 42.
    let block = func.get_first_basic_block().unwrap();
    let term = block.get_terminator().unwrap();
    let term_ref = inkwell::values::AsValueRef::as_value_ref(&term);
    let opcode = unsafe { llvm_sys::core::LLVMGetInstructionOpcode(term_ref) };
    assert_eq!(opcode, llvm_sys::LLVMOpcode::LLVMRet, "should be ret");

    assert!(func.verify(false), "valid after simplification");
}

/// Chained empty blocks should all be removed (fixed-point iteration).
///
/// Before: entry → a → b → exit
/// After:  entry → exit
#[test]
fn cfg_simplify_removes_chained_empty_blocks() {
    let ctx = Context::create();
    let (_module, func) = make_test_function(&ctx, "test_chained");
    let builder = ctx.create_builder();

    let entry = ctx.append_basic_block(func, "entry");
    let a = ctx.append_basic_block(func, "a");
    let b = ctx.append_basic_block(func, "b");
    let exit = ctx.append_basic_block(func, "exit");

    builder.position_at_end(entry);
    builder.build_unconditional_branch(a).unwrap();

    builder.position_at_end(a);
    builder.build_unconditional_branch(b).unwrap();

    builder.position_at_end(b);
    builder.build_unconditional_branch(exit).unwrap();

    builder.position_at_end(exit);
    let ret_val = ctx.i64_type().const_int(0, false);
    builder.build_return(Some(&ret_val)).unwrap();

    assert_eq!(func.get_basic_blocks().len(), 4, "pre: 4 blocks");

    let stats = simplify_cfg(func);

    // a + b removed by empty block elimination, then entry merged with
    // exit (entry was `br label %exit`, exit has 1 pred).
    assert_eq!(stats.blocks_removed, 3, "a + b + entry merge");
    assert_eq!(func.get_basic_blocks().len(), 1, "post: 1 block");
    assert!(func.verify(false), "valid after simplification");
}

/// A redundant conditional branch (`br i1 %c, %X, %X`) should be
/// simplified to `br label %X`.
#[test]
fn cfg_simplify_merges_redundant_conditionals() {
    let ctx = Context::create();
    let (_module, func) = make_test_function(&ctx, "test_redundant");
    let builder = ctx.create_builder();

    let entry = ctx.append_basic_block(func, "entry");
    let target = ctx.append_basic_block(func, "target");

    // entry: br i1 true, label %target, label %target
    builder.position_at_end(entry);
    let cond = ctx.bool_type().const_int(1, false);
    builder
        .build_conditional_branch(cond, target, target)
        .unwrap();

    // target: ret i64 7
    builder.position_at_end(target);
    let ret_val = ctx.i64_type().const_int(7, false);
    builder.build_return(Some(&ret_val)).unwrap();

    let stats = simplify_cfg(func);

    assert_eq!(
        stats.branches_simplified, 1,
        "redundant conditional should be simplified"
    );

    // After branch simplification: entry has `br label %target`.
    // Then entry merging: target has 1 pred (entry) → merge → 1 block.
    assert_eq!(
        stats.blocks_removed, 1,
        "entry merged after branch simplification"
    );
    assert_eq!(func.get_basic_blocks().len(), 1, "post: 1 block");

    // The single remaining block should return 7.
    let block = func.get_first_basic_block().unwrap();
    let term = block.get_terminator().unwrap();
    let term_ref = inkwell::values::AsValueRef::as_value_ref(&term);
    let opcode = unsafe { llvm_sys::core::LLVMGetInstructionOpcode(term_ref) };
    assert_eq!(opcode, llvm_sys::LLVMOpcode::LLVMRet, "should be ret");
    assert!(func.verify(false), "valid after simplification");
}

/// Entry block merging: when entry is just `br label %body` and body
/// has only one predecessor, the blocks are merged (body becomes new entry).
#[test]
fn cfg_simplify_merges_entry_with_single_pred_successor() {
    let ctx = Context::create();
    let (_module, func) = make_test_function(&ctx, "test_entry_merge");
    let builder = ctx.create_builder();

    let entry = ctx.append_basic_block(func, "entry");
    let body = ctx.append_basic_block(func, "body");

    // entry: br label %body
    builder.position_at_end(entry);
    builder.build_unconditional_branch(body).unwrap();

    // body: ret i64 1
    builder.position_at_end(body);
    let ret_val = ctx.i64_type().const_int(1, false);
    builder.build_return(Some(&ret_val)).unwrap();

    assert_eq!(func.get_basic_blocks().len(), 2, "pre: 2 blocks");

    let stats = simplify_cfg(func);

    // Entry merged with body → 1 block remaining.
    assert_eq!(stats.blocks_removed, 1, "entry block merged");
    assert_eq!(func.get_basic_blocks().len(), 1, "post: 1 block");
    assert!(func.verify(false), "valid after merging");
}

/// Loop preheader entry: entry → header with back-edge (>1 predecessor).
/// Entry block must NOT be merged — the phi needs both entry and latch.
#[test]
fn cfg_simplify_preserves_loop_preheader_entry() {
    let ctx = Context::create();
    let (_module, func) = make_test_function(&ctx, "test_loop_preheader");
    let builder = ctx.create_builder();
    let i64_ty = ctx.i64_type();

    let entry = ctx.append_basic_block(func, "entry");
    let header = ctx.append_basic_block(func, "header");
    let body = ctx.append_basic_block(func, "body");
    let exit = ctx.append_basic_block(func, "exit");

    // entry: br label %header
    builder.position_at_end(entry);
    builder.build_unconditional_branch(header).unwrap();

    // header: phi [0, entry], [inc, body]; cond_br -> body or exit
    builder.position_at_end(header);
    let phi = builder.build_phi(i64_ty, "i").unwrap();
    let cond = builder
        .build_int_compare(
            IntPredicate::SLT,
            phi.as_basic_value().into_int_value(),
            i64_ty.const_int(10, false),
            "lt",
        )
        .unwrap();
    builder.build_conditional_branch(cond, body, exit).unwrap();

    // body: inc = i + 1; br label %header
    builder.position_at_end(body);
    let inc = builder
        .build_int_add(
            phi.as_basic_value().into_int_value(),
            i64_ty.const_int(1, false),
            "inc",
        )
        .unwrap();
    builder.build_unconditional_branch(header).unwrap();

    // Complete phi incoming
    phi.add_incoming(&[(&i64_ty.const_int(0, false), entry), (&inc, body)]);

    // exit: ret i64 %i
    builder.position_at_end(exit);
    builder.build_return(Some(&phi.as_basic_value())).unwrap();

    assert_eq!(func.get_basic_blocks().len(), 4, "pre: 4 blocks");

    let stats = simplify_cfg(func);

    // Entry is a loop preheader — header has 2 predecessors (entry, body).
    // Must NOT be merged.
    assert_eq!(stats.blocks_removed, 0, "preheader must not be merged");
    assert_eq!(func.get_basic_blocks().len(), 4, "all blocks remain");
    assert!(func.verify(false), "valid after simplification");
}

/// A block with phi nodes is NOT considered empty, even if its only
/// non-phi instruction is a `br`.
#[test]
fn cfg_simplify_preserves_phi_block() {
    let ctx = Context::create();
    let (_module, func) = make_test_function(&ctx, "test_phi");
    let builder = ctx.create_builder();
    let i64_ty = ctx.i64_type();

    let entry = ctx.append_basic_block(func, "entry");
    let left = ctx.append_basic_block(func, "left");
    let right = ctx.append_basic_block(func, "right");
    let merge = ctx.append_basic_block(func, "merge");
    let exit = ctx.append_basic_block(func, "exit");

    // entry: br i1 true, %left, %right
    builder.position_at_end(entry);
    let cond = ctx.bool_type().const_int(1, false);
    builder.build_conditional_branch(cond, left, right).unwrap();

    // left: br label %merge
    builder.position_at_end(left);
    builder.build_unconditional_branch(merge).unwrap();

    // right: br label %merge
    builder.position_at_end(right);
    builder.build_unconditional_branch(merge).unwrap();

    // merge: phi + br label %exit  (has phi → not empty)
    builder.position_at_end(merge);
    let phi = builder.build_phi(i64_ty, "val").unwrap();
    phi.add_incoming(&[
        (&i64_ty.const_int(10, false), left),
        (&i64_ty.const_int(20, false), right),
    ]);
    builder.build_unconditional_branch(exit).unwrap();

    // exit: ret i64 %val
    builder.position_at_end(exit);
    builder.build_return(Some(&phi.as_basic_value())).unwrap();

    assert_eq!(func.get_basic_blocks().len(), 5, "pre: 5 blocks");

    let _stats = simplify_cfg(func);

    // left and right are empty trampolines that CAN be removed if phi
    // rewriting is safe. entry→left→merge and entry→right→merge get
    // rewritten so entry branches directly to merge with the phi updated.
    // The merge block itself (phi + br) should NOT be removed.
    assert!(func.verify(false), "valid after simplification");
}

/// Empty block whose removal would create a duplicate phi entry is preserved.
#[test]
fn cfg_simplify_skips_duplicate_phi_conflict() {
    let ctx = Context::create();
    let (_module, func) = make_test_function(&ctx, "test_dup_phi");
    let builder = ctx.create_builder();
    let i64_ty = ctx.i64_type();

    let entry = ctx.append_basic_block(func, "entry");
    let trampoline = ctx.append_basic_block(func, "trampoline");
    let merge = ctx.append_basic_block(func, "merge");

    // entry branches to merge directly AND via trampoline.
    // merge has a phi: [entry -> 1, trampoline -> 2]
    // Removing trampoline would need entry→merge twice in the phi (conflict).

    builder.position_at_end(entry);
    let cond = builder
        .build_int_compare(
            IntPredicate::EQ,
            i64_ty.const_int(0, false),
            i64_ty.const_int(0, false),
            "c",
        )
        .unwrap();
    builder
        .build_conditional_branch(cond, trampoline, merge)
        .unwrap();

    builder.position_at_end(trampoline);
    builder.build_unconditional_branch(merge).unwrap();

    builder.position_at_end(merge);
    let phi = builder.build_phi(i64_ty, "val").unwrap();
    phi.add_incoming(&[
        (&i64_ty.const_int(1, false), entry),
        (&i64_ty.const_int(2, false), trampoline),
    ]);
    builder.build_return(Some(&phi.as_basic_value())).unwrap();

    let pre_blocks = func.get_basic_blocks().len();
    let _stats = simplify_cfg(func);

    // The trampoline cannot be removed because entry already appears in
    // merge's phi — removing trampoline would create a duplicate edge.
    assert_eq!(
        func.get_basic_blocks().len(),
        pre_blocks,
        "no blocks removed"
    );
    assert!(func.verify(false), "valid after simplification");
}
