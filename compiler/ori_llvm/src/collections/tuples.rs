//! Tuple compilation.

use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprRange};
use ori_types::Idx;

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a tuple expression.
    pub(crate) fn compile_tuple(
        &self,
        elements: ExprRange,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get tuple elements
        let element_ids: Vec<_> = arena.get_expr_list(elements).to_vec();

        if element_ids.is_empty() {
            // Empty tuple = unit
            return None;
        }

        // Compile each element
        let mut values: Vec<BasicValueEnum<'ll>> = Vec::with_capacity(element_ids.len());
        let mut types: Vec<BasicTypeEnum<'ll>> = Vec::with_capacity(element_ids.len());

        for elem_id in &element_ids {
            if let Some(val) =
                self.compile_expr(*elem_id, arena, expr_types, locals, function, loop_ctx)
            {
                types.push(val.get_type());
                values.push(val);
            } else {
                // Element doesn't produce a value (unit element)
                // Skip for now, or could use void placeholder
                return None;
            }
        }

        // Create a struct type for the tuple
        let struct_type = self.cx().scx.type_struct(&types, false);

        // Build the struct value
        let struct_val = self.build_struct(struct_type, &values, "tuple");

        Some(struct_val.into())
    }
}
