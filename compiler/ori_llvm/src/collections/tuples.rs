//! Tuple compilation.

use std::collections::HashMap;

use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprRange, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a tuple expression.
    pub(crate) fn compile_tuple(
        &self,
        range: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get tuple elements
        let element_ids = arena.get_expr_list(range);

        if element_ids.is_empty() {
            // Empty tuple = unit
            return None;
        }

        // Compile each element
        let mut values: Vec<BasicValueEnum<'ll>> = Vec::new();
        let mut types: Vec<BasicTypeEnum<'ll>> = Vec::new();

        for &elem_id in element_ids {
            if let Some(val) = self.compile_expr(elem_id, arena, expr_types, locals, function, loop_ctx) {
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
