//! Struct compilation and field access.

use std::collections::HashMap;

use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a struct literal.
    ///
    /// For now, structs are represented as LLVM struct types with fields
    /// in declaration order. We need type information to know field order.
    pub(crate) fn compile_struct(
        &self,
        _name: Name,
        fields: ori_ir::ast::FieldInitRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get field initializers
        let field_inits = arena.get_field_inits(fields);

        if field_inits.is_empty() {
            // Empty struct = unit-like
            return None;
        }

        // Compile each field value
        let mut values: Vec<BasicValueEnum<'ll>> = Vec::new();
        let mut types: Vec<BasicTypeEnum<'ll>> = Vec::new();

        for init in field_inits {
            // Get the value - either explicit or shorthand (variable with same name)
            let value_id = init.value.unwrap_or_else(|| {
                // Shorthand: `Point { x, y }` - look up variable `x`
                // We need to find an expression for this name
                // For now, assume it's in locals
                panic!("Struct shorthand not yet supported in LLVM backend")
            });

            if let Some(val) = self.compile_expr(value_id, arena, expr_types, locals, function, loop_ctx) {
                types.push(val.get_type());
                values.push(val);
            } else {
                return None;
            }
        }

        // Create a struct type
        let struct_type = self.cx().scx.type_struct(&types, false);

        // Build the struct value
        let struct_val = self.build_struct(struct_type, &values, "struct");

        Some(struct_val.into())
    }

    /// Compile field access on a struct.
    ///
    /// For now, we need to know the field index from the type system.
    /// This is a simplified version that assumes field order matches init order.
    pub(crate) fn compile_field_access(
        &self,
        receiver: ExprId,
        field: Name,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the receiver (the struct value)
        let struct_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;

        // Get as struct value
        let struct_val = struct_val.into_struct_value();

        // For proper field access, we need the type definition to know field indices.
        // For now, use a heuristic: look up field name to get index.
        // This is a placeholder - real implementation needs type context.

        // Get field name for error messages
        let field_name = self.cx().interner.lookup(field);

        // Try common field names (x=0, y=1, z=2, etc.)
        // This is a hack - real implementation should use type info
        let field_index = match field_name {
            "x" | "first" | "0" | "a" => 0,
            "y" | "second" | "1" | "b" => 1,
            "z" | "third" | "2" | "c" => 2,
            "w" | "fourth" | "3" | "d" => 3,
            _ => {
                // Try to parse as number
                field_name.parse::<u32>().unwrap_or(0)
            }
        };

        // Extract the field value
        Some(self.extract_value(struct_val, field_index, &format!("field_{field_name}")))
    }
}
