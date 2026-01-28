//! Map compilation.

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a map literal.
    pub(crate) fn compile_map(
        &self,
        entries: ori_ir::ast::MapEntryRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let map_entries = arena.get_map_entries(entries);

        if map_entries.is_empty() {
            // Empty map - return struct with zeros
            let map_type = self.cx().map_type();
            let zero = self.cx().scx.type_i64().const_int(0, false);
            let null_ptr = self.cx().scx.type_ptr().const_null();

            let map_val = self.build_struct(
                map_type,
                &[zero.into(), zero.into(), null_ptr.into(), null_ptr.into()],
                "empty_map",
            );

            return Some(map_val.into());
        }

        // Compile each key-value pair
        let mut keys: Vec<BasicValueEnum<'ll>> = Vec::new();
        let mut vals: Vec<BasicValueEnum<'ll>> = Vec::new();

        for entry in map_entries {
            if let Some(key) = self.compile_expr(entry.key, arena, expr_types, locals, function, loop_ctx) {
                if let Some(val) = self.compile_expr(entry.value, arena, expr_types, locals, function, loop_ctx) {
                    keys.push(key);
                    vals.push(val);
                }
            }
        }

        if keys.is_empty() {
            return None;
        }

        let len = keys.len() as u64;

        // For simplicity, create a map struct with the length
        // A real implementation would use a hash table
        let map_type = self.cx().map_type();
        let len_val = self.cx().scx.type_i64().const_int(len, false);
        let null_ptr = self.cx().scx.type_ptr().const_null();

        let map_val = self.build_struct(
            map_type,
            &[len_val.into(), len_val.into(), null_ptr.into(), null_ptr.into()],
            "map",
        );

        Some(map_val.into())
    }
}
