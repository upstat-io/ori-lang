//! Index expression compilation.

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId};
use ori_types::Idx;
use tracing::instrument;

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile an index expression: receiver[index]
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "trace"
    )]
    pub(crate) fn compile_index(
        &self,
        receiver: ExprId,
        index: ExprId,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let recv_val =
            self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;
        let idx_val = self.compile_expr(index, arena, expr_types, locals, function, loop_ctx)?;

        // Handle different receiver types
        match recv_val {
            BasicValueEnum::StructValue(struct_val) => {
                // Could be a tuple - use index as field number
                // Only works with integer indices
                let BasicValueEnum::IntValue(idx) = idx_val else {
                    return self.emit_not_implemented("non-integer indexing (maps)");
                };
                idx.get_zero_extended_constant()
                    .and_then(|const_idx| self.extract_value(struct_val, const_idx as u32, "index"))
            }
            _ => self.emit_not_implemented("dynamic list indexing"),
        }
    }
}
