//! List compilation.

use std::collections::HashMap;

use inkwell::types::BasicType;
use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprList, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a list literal.
    /// Lists are represented as { i64 len, i64 cap, ptr data }.
    pub(crate) fn compile_list(
        &self,
        list: ExprList,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let elements: Vec<_> = arena.iter_expr_list(list).collect();

        if elements.is_empty() {
            // Empty list - return struct with zeros
            let list_type = self.cx().list_type();
            let zero = self.cx().scx.type_i64().const_int(0, false);
            let null_ptr = self.cx().scx.type_ptr().const_null();

            let list_val = self.build_struct(
                list_type,
                &[zero.into(), zero.into(), null_ptr.into()],
                "empty_list",
            );

            return Some(list_val.into());
        }

        // Compile each element
        let mut values: Vec<BasicValueEnum<'ll>> = Vec::new();
        for elem_id in &elements {
            if let Some(val) =
                self.compile_expr(*elem_id, arena, expr_types, locals, function, loop_ctx)
            {
                values.push(val);
            }
        }

        if values.is_empty() {
            return None;
        }

        // Get element type from first value
        let elem_type = values[0].get_type();
        let len = values.len() as u64;

        // Create array type for storage
        let array_type = elem_type.array_type(len as u32);

        // Allocate array on stack (for now - runtime would use heap)
        let array_ptr = self.alloca(array_type.into(), "list_storage");

        // Store each element
        for (i, val) in values.iter().enumerate() {
            let indices = [
                self.cx().scx.type_i64().const_int(0, false),
                self.cx().scx.type_i64().const_int(i as u64, false),
            ];
            let elem_ptr = self.gep(array_type.into(), array_ptr, &indices, "elem_ptr");
            self.store(*val, elem_ptr);
        }

        // Create list struct
        let list_type = self.cx().list_type();
        let len_val = self.cx().scx.type_i64().const_int(len, false);

        let list_val = self.build_struct(
            list_type,
            &[len_val.into(), len_val.into(), array_ptr.into()],
            "list",
        );

        Some(list_val.into())
    }
}
