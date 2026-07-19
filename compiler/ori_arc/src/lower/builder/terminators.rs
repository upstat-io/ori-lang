//! Terminator emission for in-progress ARC basic blocks.

use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcTerminator, ArcVarId};

use super::{ArcIrBuilder, InvokeTargets};

impl ArcIrBuilder {
    /// Terminate with `Return`.
    pub(crate) fn terminate_return(&mut self, value: ArcVarId) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Return { value });
    }

    /// Terminate with unconditional `Jump`.
    pub(crate) fn terminate_jump(&mut self, target: ArcBlockId, args: Vec<ArcVarId>) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Jump { target, args });
    }

    /// Terminate with conditional `Branch`.
    pub(crate) fn terminate_branch(
        &mut self,
        cond: ArcVarId,
        then_block: ArcBlockId,
        else_block: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Branch {
            cond,
            then_block,
            else_block,
        });
    }

    /// Terminate with multi-way `Switch`.
    pub(crate) fn terminate_switch(
        &mut self,
        scrutinee: ArcVarId,
        cases: Vec<(u64, ArcBlockId)>,
        default: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Switch {
            scrutinee,
            cases,
            default,
        });
    }

    /// Terminate with `Invoke` (function call that may unwind).
    ///
    /// The `dst` variable is defined at the `normal` continuation block's
    /// entry, NOT in the current block. The `unwind` block receives control
    /// if the callee unwinds (panics).
    pub(crate) fn terminate_invoke(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        targets: InvokeTargets,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        let arg_count = args.len();
        block.terminator = Some(ArcTerminator::Invoke {
            dst,
            ty,
            func,
            args,
            arg_ownership: vec![crate::ir::ArgOwnership::Owned; arg_count],
            mono_instance_id: targets.mono_instance_id,
            normal: targets.normal,
            unwind: targets.unwind,
        });
    }

    /// Terminate with `InvokeIndirect` (indirect call through closure that may unwind).
    pub(crate) fn terminate_invoke_indirect(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        normal: ArcBlockId,
        unwind: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::InvokeIndirect {
            dst,
            ty,
            closure,
            args,
            arg_ownership: Vec::new(),
            normal,
            unwind,
        });
    }

    /// Terminate with `Resume` (re-raise an unwinding panic).
    pub(crate) fn terminate_resume(&mut self) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Resume);
    }

    /// Terminate with `Unreachable`.
    pub(crate) fn terminate_unreachable(&mut self) {
        let block = &mut self.blocks[self.current_block.index()];
        assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Unreachable);
    }
}
