//! Function sequence patterns (run, try, match).

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::IntPredicate;
use ori_ir::ast::patterns::{BindingPattern, FunctionSeq, SeqBinding};
use ori_ir::{ExprArena, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a FunctionSeq (run, try, match).
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

    /// Compile a SeqBinding (let or stmt).
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

    /// Compile a SeqBinding in a try context.
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

    /// Bind a pattern to a value.
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
                // Discard
            }
            _ => {
                // TODO: handle other patterns
            }
        }
    }
}
