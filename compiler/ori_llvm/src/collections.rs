//! Collection and value construction: tuples, structs, lists, maps, ranges,
//! strings, Option/Result constructors, and indexing.

use std::collections::HashMap;

use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, ExprRange, Name, TypeId};

use crate::{LLVMCodegen, LoopContext};

impl<'ctx> LLVMCodegen<'ctx> {
    /// Compile a tuple expression.
    pub(crate) fn compile_tuple(
        &self,
        range: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get tuple elements
        let element_ids = arena.get_expr_list(range);

        if element_ids.is_empty() {
            // Empty tuple = unit
            return None;
        }

        // Compile each element
        let mut values: Vec<BasicValueEnum<'ctx>> = Vec::new();
        let mut types: Vec<BasicTypeEnum<'ctx>> = Vec::new();

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
        let struct_type = self.context.struct_type(&types, false);

        // Build the struct value
        let mut struct_val = struct_type.get_undef();
        for (i, val) in values.into_iter().enumerate() {
            struct_val = self.builder
                .build_insert_value(struct_val, val, i as u32, "tuple_elem")
                .ok()?
                .into_struct_value();
        }

        Some(struct_val.into())
    }

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
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get field initializers
        let field_inits = arena.get_field_inits(fields);

        if field_inits.is_empty() {
            // Empty struct = unit-like
            return None;
        }

        // Compile each field value
        let mut values: Vec<BasicValueEnum<'ctx>> = Vec::new();
        let mut types: Vec<BasicTypeEnum<'ctx>> = Vec::new();

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
        let struct_type = self.context.struct_type(&types, false);

        // Build the struct value
        let mut struct_val = struct_type.get_undef();
        for (i, val) in values.into_iter().enumerate() {
            struct_val = self.builder
                .build_insert_value(struct_val, val, i as u32, "struct_field")
                .ok()?
                .into_struct_value();
        }

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
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the receiver (the struct value)
        let struct_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;

        // Get as struct value
        let struct_val = struct_val.into_struct_value();

        // For proper field access, we need the type definition to know field indices.
        // For now, use a heuristic: look up field name to get index.
        // This is a placeholder - real implementation needs type context.

        // Get field name for error messages
        let field_name = self.interner.lookup(field);

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
        self.builder
            .build_extract_value(struct_val, field_index, &format!("field_{field_name}"))
            .ok()
    }

    /// Compile Some(value).
    pub(crate) fn compile_some(
        &self,
        inner: ExprId,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the inner value
        let inner_val = self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?;

        // Create Option type with this payload type
        let opt_type = self.option_type(inner_val.get_type());

        // Build the struct: { tag = 1, value = inner_val }
        let tag = self.context.i8_type().const_int(1, false); // 1 = Some

        let mut struct_val = opt_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "some_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, inner_val, 1, "some_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile None.
    pub(crate) fn compile_none(&self, type_id: TypeId) -> Option<BasicValueEnum<'ctx>> {
        // For None, we need to know the inner type to create the right struct.
        // Since we don't have that info easily, use i64 as default payload.
        let payload_type = self.llvm_type(type_id);

        // If we got a pointer type (unknown), use i64 as default
        let payload_type = if payload_type.is_pointer_type() {
            self.context.i64_type().into()
        } else {
            payload_type
        };

        let opt_type = self.option_type(payload_type);

        // Build the struct: { tag = 0, value = undef }
        let tag = self.context.i8_type().const_int(0, false); // 0 = None
        let default_val = self.default_value_for_type(payload_type);

        let mut struct_val = opt_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "none_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, default_val, 1, "none_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile Ok(value).
    pub(crate) fn compile_ok(
        &self,
        inner: Option<ExprId>,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get the inner value (or use unit if None)
        let inner_val = if let Some(inner_id) = inner {
            self.compile_expr(inner_id, arena, expr_types, locals, function, loop_ctx)?
        } else {
            // Ok() with no value - use a dummy i64
            self.context.i64_type().const_int(0, false).into()
        };

        // Create Result type with this payload type
        let result_type = self.result_type(inner_val.get_type());

        // Build the struct: { tag = 0, value = inner_val }
        let tag = self.context.i8_type().const_int(0, false); // 0 = Ok

        let mut struct_val = result_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "ok_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, inner_val, 1, "ok_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile Err(value).
    pub(crate) fn compile_err(
        &self,
        inner: Option<ExprId>,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get the inner value (or use unit if None)
        let inner_val = if let Some(inner_id) = inner {
            self.compile_expr(inner_id, arena, expr_types, locals, function, loop_ctx)?
        } else {
            // Err() with no value - use a dummy i64
            self.context.i64_type().const_int(0, false).into()
        };

        // Create Result type with this payload type
        let result_type = self.result_type(inner_val.get_type());

        // Build the struct: { tag = 1, value = inner_val }
        let tag = self.context.i8_type().const_int(1, false); // 1 = Err

        let mut struct_val = result_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "err_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, inner_val, 1, "err_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile a string literal.
    ///
    /// Creates a global constant string and returns a pointer to it.
    /// Strings are represented as { i64 len, i8* data } structs.
    pub(crate) fn compile_string(&self, name: Name) -> Option<BasicValueEnum<'ctx>> {
        let string_content = self.interner.lookup(name);

        // Create a unique global name for this string based on a hash
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        string_content.hash(&mut hasher);
        let global_name = format!(".str.{:x}", hasher.finish());

        // Check if we already have this string as a global
        if let Some(global) = self.module.get_global(&global_name) {
            // Return pointer to existing string data
            let ptr = global.as_pointer_value();

            // Create string struct { len, data_ptr }
            let len = self.context.i64_type().const_int(string_content.len() as u64, false);
            let string_struct = self.string_type();

            let mut struct_val = string_struct.get_undef();
            struct_val = self.builder
                .build_insert_value(struct_val, len, 0, "str_len")
                .ok()?
                .into_struct_value();
            struct_val = self.builder
                .build_insert_value(struct_val, ptr, 1, "str_data")
                .ok()?
                .into_struct_value();

            return Some(struct_val.into());
        }

        // Create a null-terminated string constant
        let string_bytes: Vec<u8> = string_content.bytes().chain(std::iter::once(0)).collect();
        let string_const = self.context.const_string(&string_bytes, false);

        // Create global variable for the string data
        let global = self.module.add_global(string_const.get_type(), None, &global_name);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_constant(true);
        global.set_initializer(&string_const);

        // Get pointer to the string data
        let ptr = global.as_pointer_value();

        // Create string struct { len, data_ptr }
        let len = self.context.i64_type().const_int(string_content.len() as u64, false);
        let string_struct = self.string_type();

        let mut struct_val = string_struct.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, len, 0, "str_len")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, ptr, 1, "str_data")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile a list literal.
    /// Lists are represented as { i64 len, i64 cap, ptr data }.
    pub(crate) fn compile_list(
        &self,
        range: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let elements = arena.get_expr_list(range);

        if elements.is_empty() {
            // Empty list - return struct with zeros
            let list_type = self.list_type();
            let zero = self.context.i64_type().const_int(0, false);
            let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();

            let mut list_val = list_type.get_undef();
            list_val = self.builder.build_insert_value(list_val, zero, 0, "list_len").ok()?.into_struct_value();
            list_val = self.builder.build_insert_value(list_val, zero, 1, "list_cap").ok()?.into_struct_value();
            list_val = self.builder.build_insert_value(list_val, null_ptr, 2, "list_data").ok()?.into_struct_value();

            return Some(list_val.into());
        }

        // Compile each element
        let mut values: Vec<BasicValueEnum<'ctx>> = Vec::new();
        for &elem_id in elements {
            if let Some(val) = self.compile_expr(elem_id, arena, expr_types, locals, function, loop_ctx) {
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
        let array_ptr = self.builder.build_alloca(array_type, "list_storage").ok()?;

        // Store each element
        for (i, val) in values.iter().enumerate() {
            let indices = [
                self.context.i64_type().const_int(0, false),
                self.context.i64_type().const_int(i as u64, false),
            ];
            // SAFETY: GEP with constant indices into an array we just allocated
            #[allow(unsafe_code)]
            let elem_ptr = unsafe {
                self.builder.build_gep(array_type, array_ptr, &indices, "elem_ptr").ok()?
            };
            self.builder.build_store(elem_ptr, *val).ok()?;
        }

        // Create list struct
        let list_type = self.list_type();
        let len_val = self.context.i64_type().const_int(len, false);

        let mut list_val = list_type.get_undef();
        list_val = self.builder.build_insert_value(list_val, len_val, 0, "list_len").ok()?.into_struct_value();
        list_val = self.builder.build_insert_value(list_val, len_val, 1, "list_cap").ok()?.into_struct_value();
        list_val = self.builder.build_insert_value(list_val, array_ptr, 2, "list_data").ok()?.into_struct_value();

        Some(list_val.into())
    }

    /// Compile a map literal.
    pub(crate) fn compile_map(
        &self,
        entries: ori_ir::ast::MapEntryRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let map_entries = arena.get_map_entries(entries);

        if map_entries.is_empty() {
            // Empty map - return struct with zeros
            let map_type = self.map_type();
            let zero = self.context.i64_type().const_int(0, false);
            let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();

            let mut map_val = map_type.get_undef();
            map_val = self.builder.build_insert_value(map_val, zero, 0, "map_len").ok()?.into_struct_value();
            map_val = self.builder.build_insert_value(map_val, zero, 1, "map_cap").ok()?.into_struct_value();
            map_val = self.builder.build_insert_value(map_val, null_ptr, 2, "map_keys").ok()?.into_struct_value();
            map_val = self.builder.build_insert_value(map_val, null_ptr, 3, "map_vals").ok()?.into_struct_value();

            return Some(map_val.into());
        }

        // Compile each key-value pair
        let mut keys: Vec<BasicValueEnum<'ctx>> = Vec::new();
        let mut vals: Vec<BasicValueEnum<'ctx>> = Vec::new();

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
        let map_type = self.map_type();
        let len_val = self.context.i64_type().const_int(len, false);
        let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();

        let mut map_val = map_type.get_undef();
        map_val = self.builder.build_insert_value(map_val, len_val, 0, "map_len").ok()?.into_struct_value();
        map_val = self.builder.build_insert_value(map_val, len_val, 1, "map_cap").ok()?.into_struct_value();
        map_val = self.builder.build_insert_value(map_val, null_ptr, 2, "map_keys").ok()?.into_struct_value();
        map_val = self.builder.build_insert_value(map_val, null_ptr, 3, "map_vals").ok()?.into_struct_value();

        Some(map_val.into())
    }

    /// Compile a range expression.
    /// Ranges are represented as { i64 start, i64 end, i1 inclusive }.
    pub(crate) fn compile_range(
        &self,
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile start (default to 0)
        let start_val = if let Some(start_id) = start {
            self.compile_expr(start_id, arena, expr_types, locals, function, loop_ctx)?
                .into_int_value()
        } else {
            self.context.i64_type().const_int(0, false)
        };

        // Compile end (default to i64::MAX)
        let end_val = if let Some(end_id) = end {
            self.compile_expr(end_id, arena, expr_types, locals, function, loop_ctx)?
                .into_int_value()
        } else {
            self.context.i64_type().const_int(i64::MAX as u64, false)
        };

        // Create range struct
        let range_type = self.range_type();
        let inclusive_val = self.context.bool_type().const_int(u64::from(inclusive), false);

        let mut range_val = range_type.get_undef();
        range_val = self.builder.build_insert_value(range_val, start_val, 0, "range_start").ok()?.into_struct_value();
        range_val = self.builder.build_insert_value(range_val, end_val, 1, "range_end").ok()?.into_struct_value();
        range_val = self.builder.build_insert_value(range_val, inclusive_val, 2, "range_incl").ok()?.into_struct_value();

        Some(range_val.into())
    }

    /// Compile an index expression: receiver[index]
    pub(crate) fn compile_index(
        &self,
        receiver: ExprId,
        index: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;
        let idx_val = self.compile_expr(index, arena, expr_types, locals, function, loop_ctx)?;

        // Handle different receiver types
        match recv_val {
            BasicValueEnum::StructValue(struct_val) => {
                // Could be a tuple - use index as field number
                let idx = idx_val.into_int_value();
                if let Some(const_idx) = idx.get_zero_extended_constant() {
                    self.builder
                        .build_extract_value(struct_val, const_idx as u32, "index")
                        .ok()
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
