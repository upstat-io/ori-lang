//! Index expression compilation.

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile an index expression: receiver[index]
    pub(crate) fn compile_index(
        &self,
        receiver: ExprId,
        index: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;
        let idx_val = self.compile_expr(index, arena, expr_types, locals, function, loop_ctx)?;

        // Handle different receiver types
        match recv_val {
            BasicValueEnum::StructValue(struct_val) => {
                // Could be a tuple - use index as field number
                let idx = idx_val.into_int_value();
                if let Some(const_idx) = idx.get_zero_extended_constant() {
                    Some(self.extract_value(struct_val, const_idx as u32, "index"))
                } else {
                    // Dynamic index not supported for tuples
                    None
                }
            }
            _ => {
                // For lists/arrays, would need GEP or runtime call
                // Return None for now
                None
            }
        }
    }
}
