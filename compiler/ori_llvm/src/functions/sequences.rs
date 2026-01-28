//! Function sequence patterns (run, try, match).

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::IntPredicate;
use ori_ir::ast::patterns::{BindingPattern, FunctionSeq, SeqBinding};
use ori_ir::{ExprArena, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a `FunctionSeq` (run, try, match).
    pub(crate) fn compile_function_seq(
        &self,
        seq: &FunctionSeq,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        match seq {
            FunctionSeq::Run {
                bindings, result, ..
            } => {
                // Execute bindings sequentially
                let seq_bindings = arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    self.compile_seq_binding(
                        binding, arena, expr_types, locals, function, loop_ctx,
                    );
                }
                // Return result
                self.compile_expr(*result, arena, expr_types, locals, function, loop_ctx)
            }

            FunctionSeq::Try {
                bindings, result, ..
            } => {
                // Execute bindings with error propagation
                let seq_bindings = arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    // Compile binding with try semantics (unwrap Result, propagate errors)
                    self.compile_try_binding(
                        binding, arena, expr_types, locals, function, loop_ctx,
                    )?;
                }
                self.compile_expr(*result, arena, expr_types, locals, function, loop_ctx)
            }

            FunctionSeq::Match {
                scrutinee, arms, ..
            } => {
                // Delegate to existing match compilation
                self.compile_match(
                    *scrutinee,
                    *arms,
                    result_type,
                    arena,
                    expr_types,
                    locals,
                    function,
                    loop_ctx,
                )
            }

            FunctionSeq::ForPattern {
                over,
                map,
                arm: _,
                default,
                ..
            } => {
                // Compile the for pattern
                let iter_val =
                    self.compile_expr(*over, arena, expr_types, locals, function, loop_ctx)?;

                // Apply map if present
                let _mapped = if let Some(map_fn) = map {
                    self.compile_expr(*map_fn, arena, expr_types, locals, function, loop_ctx)?
                } else {
                    iter_val
                };

                // For now, just return the default
                self.compile_expr(*default, arena, expr_types, locals, function, loop_ctx)
            }
        }
    }

    /// Compile a `SeqBinding` (let or stmt).
    fn compile_seq_binding(
        &self,
        binding: &SeqBinding,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        match binding {
            SeqBinding::Let { pattern, value, .. } => self.compile_let(
                pattern, *value, arena, expr_types, locals, function, loop_ctx,
            ),
            SeqBinding::Stmt { expr, .. } => {
                self.compile_expr(*expr, arena, expr_types, locals, function, loop_ctx)
            }
        }
    }

    /// Compile a `SeqBinding` in a try context.
    ///
    /// For let bindings, if the value is a Result, unwrap it and propagate errors.
    /// For statements, just evaluate and check for errors if it's a Result.
    fn compile_try_binding(
        &self,
        binding: &SeqBinding,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        match binding {
            SeqBinding::Let { pattern, value, .. } => {
                // Compile the value expression
                let result_val =
                    self.compile_expr(*value, arena, expr_types, locals, function, loop_ctx)?;

                // Check if the value is a struct (Result/Option type)
                if let BasicValueEnum::StructValue(struct_val) = result_val {
                    // Check if this looks like a Result (has tag in first field)
                    // We assume Result structs have { i8 tag, T value } layout
                    if struct_val.get_type().count_fields() == 2 {
                        // Extract tag to check if Ok or Err
                        let tag = self
                            .extract_value(struct_val, 0, "try_tag")
                            .into_int_value();

                        // Check if Ok (tag == 0)
                        let is_ok = self.icmp(
                            IntPredicate::EQ,
                            tag,
                            self.cx().scx.type_i8().const_int(0, false),
                            "is_ok",
                        );

                        // Create blocks for Ok and Err paths
                        let ok_bb = self.append_block(function, "try_ok");
                        let err_bb = self.append_block(function, "try_err");
                        let cont_bb = self.append_block(function, "try_cont");

                        self.cond_br(is_ok, ok_bb, err_bb);

                        // Err path: early return with the error
                        self.position_at_end(err_bb);
                        self.ret(result_val);

                        // Ok path: extract value and continue
                        self.position_at_end(ok_bb);
                        let inner_val = self.extract_value(struct_val, 1, "ok_val");
                        self.br(cont_bb);

                        // Continue block
                        self.position_at_end(cont_bb);

                        // Bind the unwrapped value to the pattern
                        self.bind_pattern(pattern, inner_val, locals);

                        return Some(inner_val);
                    }
                }

                // Not a Result type - bind directly
                self.bind_pattern(pattern, result_val, locals);
                Some(result_val)
            }
            SeqBinding::Stmt { expr, .. } => {
                self.compile_expr(*expr, arena, expr_types, locals, function, loop_ctx)
            }
        }
    }

    /// Bind a pattern to a value, populating locals.
    pub(crate) fn bind_pattern(
        &self,
        pattern: &BindingPattern,
        value: BasicValueEnum<'ll>,
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
    ) {
        match pattern {
            BindingPattern::Name(name) => {
                locals.insert(*name, value);
            }
            BindingPattern::Wildcard => {
                // Discard the value
            }
            BindingPattern::Tuple(patterns) => {
                // Extract each tuple element by index
                if let BasicValueEnum::StructValue(struct_val) = value {
                    for (i, pat) in patterns.iter().enumerate() {
                        let elem = self.extract_value(struct_val, i as u32, &format!("tuple_{i}"));
                        self.bind_pattern(pat, elem, locals);
                    }
                }
            }
            BindingPattern::Struct { fields } => {
                // Extract each struct field by index
                if let BasicValueEnum::StructValue(struct_val) = value {
                    for (field_name, inner_pattern) in fields {
                        let field_name_str = self.cx().interner.lookup(*field_name);
                        let field_index = self.field_name_to_index(field_name_str);
                        let field_val = self.extract_value(
                            struct_val,
                            field_index,
                            &format!("field_{field_name_str}"),
                        );

                        // If there's an inner pattern (rename), bind to that; otherwise bind to field name
                        if let Some(inner) = inner_pattern {
                            self.bind_pattern(inner, field_val, locals);
                        } else {
                            // Shorthand: { x } binds field x to variable x
                            locals.insert(*field_name, field_val);
                        }
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                // Lists are { i64 len, i64 cap, ptr data }
                if let BasicValueEnum::StructValue(list_struct) = value {
                    // Extract the data pointer (index 2)
                    let data_ptr = self.extract_value(list_struct, 2, "list_data");

                    // Extract each element by loading from the array
                    for (i, pat) in elements.iter().enumerate() {
                        let indices = [
                            self.cx().scx.type_i64().const_int(0, false),
                            self.cx().scx.type_i64().const_int(i as u64, false),
                        ];

                        // Assume i64 elements for now - proper implementation would use type info
                        let elem_type = self.cx().scx.type_i64();
                        let array_type = elem_type.array_type(elements.len() as u32);

                        let elem_ptr = self.gep(
                            array_type.into(),
                            data_ptr.into_pointer_value(),
                            &indices,
                            &format!("elem_{i}_ptr"),
                        );
                        let elem_val = self.load(elem_type.into(), elem_ptr, &format!("elem_{i}"));
                        self.bind_pattern(pat, elem_val, locals);
                    }

                    // Handle rest pattern (..rest)
                    if let Some(rest_name) = rest {
                        // For now, bind the remaining elements as a new list
                        // This is simplified - real implementation would create a slice
                        let len_val = self.extract_value(list_struct, 0, "list_len");
                        let consumed = self
                            .cx()
                            .scx
                            .type_i64()
                            .const_int(elements.len() as u64, false);
                        let rest_len = self.sub(len_val.into_int_value(), consumed, "rest_len");

                        // Create a new list struct for the rest
                        // Offset the data pointer by the consumed elements
                        let elem_type = self.cx().scx.type_i64();
                        let offset_indices = [
                            self.cx().scx.type_i64().const_int(0, false),
                            self.cx()
                                .scx
                                .type_i64()
                                .const_int(elements.len() as u64, false),
                        ];
                        let array_type = elem_type.array_type(1); // Dummy size for GEP
                        let rest_ptr = self.gep(
                            array_type.into(),
                            data_ptr.into_pointer_value(),
                            &offset_indices,
                            "rest_data",
                        );

                        let list_type = self.cx().list_type();
                        let rest_list = self.build_struct(
                            list_type,
                            &[rest_len.into(), rest_len.into(), rest_ptr.into()],
                            "rest_list",
                        );

                        locals.insert(*rest_name, rest_list.into());
                    }
                }
            }
        }
    }

    /// Map field name to index using the same heuristic as field access.
    #[expect(clippy::unused_self, reason = "consistent method signature pattern")]
    fn field_name_to_index(&self, name: &str) -> u32 {
        match name {
            "x" | "first" | "0" | "a" => 0,
            "y" | "second" | "1" | "b" => 1,
            "z" | "third" | "2" | "c" => 2,
            "w" | "fourth" | "3" | "d" => 3,
            _ => name.parse::<u32>().unwrap_or(0),
        }
    }
}
