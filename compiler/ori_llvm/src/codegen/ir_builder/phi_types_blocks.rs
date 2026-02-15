//! Phi nodes, type registration, block/function management for `IrBuilder`.

use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};

use super::IrBuilder;
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

impl<'ctx> IrBuilder<'_, 'ctx> {
    // -- Phi nodes --

    /// Build an empty phi node.
    ///
    /// The caller must add incoming values afterwards using the returned
    /// `ValueId`. Note: the underlying inkwell `PhiValue` is stored as a
    /// `BasicValueEnum` (via `as_basic_value()`).
    pub fn phi(&mut self, ty: LLVMTypeId, name: &str) -> ValueId {
        let llvm_ty = self.arena.get_type(ty);
        let phi = self.builder.build_phi(llvm_ty, name).expect("phi");
        self.arena.push_value(phi.as_basic_value())
    }

    /// Add incoming values to a phi node.
    ///
    /// The `phi` parameter must be a `ValueId` returned by `self.phi()`.
    /// We reconstruct the `PhiValue` from the stored LLVM value ref.
    pub fn add_phi_incoming(&mut self, phi: ValueId, incoming: &[(ValueId, BlockId)]) {
        use inkwell::values::AsValueRef;

        let phi_val = self.arena.get_value(phi);

        // SAFETY: `phi_val` was created by `build_phi` and stored via
        // `as_basic_value()`. The underlying LLVMValueRef is still a phi.
        let raw_phi = unsafe { inkwell::values::PhiValue::new(phi_val.as_value_ref()) };

        // Collect values and blocks into owned Vecs so we can borrow them.
        let vals: Vec<BasicValueEnum<'ctx>> = incoming
            .iter()
            .map(|&(v, _)| self.arena.get_value(v))
            .collect();
        let blocks: Vec<inkwell::basic_block::BasicBlock<'ctx>> = incoming
            .iter()
            .map(|&(_, b)| self.arena.get_block(b))
            .collect();

        // Build the &[(&dyn BasicValue, BasicBlock)] slice that inkwell expects.
        let refs: Vec<(
            &dyn BasicValue<'ctx>,
            inkwell::basic_block::BasicBlock<'ctx>,
        )> = vals
            .iter()
            .zip(blocks.iter())
            .map(|(v, &b)| (v as &dyn BasicValue<'ctx>, b))
            .collect();
        raw_phi.add_incoming(&refs);
    }

    /// Build a phi from a list of incoming (value, block) pairs.
    ///
    /// Optimizations:
    /// - 0 incoming → returns `None`
    /// - 1 incoming → returns the value directly (no phi needed)
    /// - 2+ incoming → creates a real phi node
    pub fn phi_from_incoming(
        &mut self,
        ty: LLVMTypeId,
        incoming: &[(ValueId, BlockId)],
        name: &str,
    ) -> Option<ValueId> {
        match incoming.len() {
            0 => None,
            1 => Some(incoming[0].0),
            _ => {
                let phi_id = self.phi(ty, name);
                self.add_phi_incoming(phi_id, incoming);
                Some(phi_id)
            }
        }
    }

    // -- Type registration --

    /// Register an LLVM type in the arena.
    pub fn register_type(&mut self, ty: BasicTypeEnum<'ctx>) -> LLVMTypeId {
        self.arena.push_type(ty)
    }

    /// Register and return the `i1` (bool) type ID.
    #[inline]
    pub fn bool_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i1().into())
    }

    /// Register and return the `i8` type ID.
    #[inline]
    pub fn i8_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i8().into())
    }

    /// Register and return the `i32` type ID.
    #[inline]
    pub fn i32_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i32().into())
    }

    /// Register and return the `i64` type ID.
    #[inline]
    pub fn i64_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i64().into())
    }

    /// Register and return the `f64` type ID.
    #[inline]
    pub fn f64_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_f64().into())
    }

    /// Register and return the opaque pointer type ID.
    #[inline]
    pub fn ptr_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_ptr().into())
    }

    /// Register and return the unit type ID (i64, matching Ori convention).
    #[inline]
    pub fn unit_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i64().into())
    }

    /// Register and return the closure fat-pointer type `{ ptr, ptr }`.
    ///
    /// All function-typed values use this two-pointer representation:
    /// field 0 = function pointer, field 1 = environment pointer (null if
    /// no captures).
    pub fn closure_type(&mut self) -> LLVMTypeId {
        let struct_ty = self.scx.type_struct(
            &[self.scx.type_ptr().into(), self.scx.type_ptr().into()],
            false,
        );
        self.arena.push_type(struct_ty.into())
    }

    // -- Block management --

    /// Append a new basic block to a function.
    pub fn append_block(&mut self, function: FunctionId, name: &str) -> BlockId {
        let func = self.arena.get_function(function);
        let bb = self.scx.llcx.append_basic_block(func, name);
        self.arena.push_block(bb)
    }

    /// Position the builder at the end of a basic block.
    pub fn position_at_end(&mut self, block: BlockId) {
        let bb = self.arena.get_block(block);
        self.builder.position_at_end(bb);
        self.current_block = Some(block);
    }

    /// Get the current basic block ID (if any).
    #[inline]
    pub fn current_block(&self) -> Option<BlockId> {
        self.current_block
    }

    /// Check if the current block is already terminated.
    pub fn current_block_terminated(&self) -> bool {
        self.current_block
            .is_some_and(|id| self.arena.get_block(id).get_terminator().is_some())
    }

    // -- Position management --

    /// Save the current builder position, returning the block ID.
    ///
    /// Call `restore_position` with the returned ID to restore.
    /// This uses the manual save/restore pattern to avoid borrow checker
    /// friction with RAII guards and `&mut self`.
    #[inline]
    pub fn save_position(&self) -> Option<BlockId> {
        self.current_block
    }

    /// Restore builder position to a previously saved block.
    pub fn restore_position(&mut self, saved: Option<BlockId>) {
        if let Some(block_id) = saved {
            let bb = self.arena.get_block(block_id);
            self.builder.position_at_end(bb);
            self.current_block = Some(block_id);
        }
    }

    // -- Function management --

    /// Set the currently-active function.
    pub fn set_current_function(&mut self, func: FunctionId) {
        self.current_function = Some(func);
    }

    /// Get the currently-active function ID.
    #[inline]
    pub fn current_function(&self) -> Option<FunctionId> {
        self.current_function
    }

    /// Get the inkwell `FunctionValue` for the currently-active function.
    pub fn current_function_value(&self) -> Option<FunctionValue<'ctx>> {
        self.current_function.map(|id| self.arena.get_function(id))
    }

    /// Get the inkwell `FunctionValue` for any function ID.
    pub fn get_function_value(&self, id: FunctionId) -> FunctionValue<'ctx> {
        self.arena.get_function(id)
    }

    /// Get a function parameter as a `ValueId`.
    ///
    /// `param_index` is the LLVM-level parameter index (0-based, includes
    /// hidden sret parameter if present).
    pub fn get_param(&mut self, func: FunctionId, param_index: u32) -> ValueId {
        let func_val = self.arena.get_function(func);
        let Some(param) = func_val.get_nth_param(param_index) else {
            tracing::error!(
                func = %func_val.get_name().to_string_lossy(),
                param_index,
                param_count = func_val.count_params(),
                "parameter index out of bounds — returning zero"
            );
            self.record_codegen_error();
            return self.const_i64(0);
        };
        self.arena.push_value(param)
    }

    /// Set the debug name of a value.
    pub fn set_value_name(&self, val: ValueId, name: &str) {
        let v = self.arena.get_value(val);
        v.set_name(name);
    }

    /// Check if a specific block has a terminator instruction.
    pub fn block_has_terminator(&self, block: BlockId) -> bool {
        self.arena.get_block(block).get_terminator().is_some()
    }
}
