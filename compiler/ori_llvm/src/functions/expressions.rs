//! Named expression patterns (recurse, parallel, etc.).

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::patterns::FunctionExp;
use ori_ir::{ExprArena, TypeId};

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a `FunctionExp` (recurse, parallel, etc.).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compile_function_exp(
        &self,
        exp: &FunctionExp,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        use ori_ir::ast::patterns::FunctionExpKind;

        let named_exprs = arena.get_named_exprs(exp.props);

        match exp.kind {
            FunctionExpKind::Recurse => {
                // Find condition, base, and step
                let mut condition = None;
                let mut base = None;
                let mut step = None;

                for ne in named_exprs {
                    let name = self.cx().interner.lookup(ne.name);
                    match name {
                        "condition" => condition = Some(ne.value),
                        "base" => base = Some(ne.value),
                        "step" => step = Some(ne.value),
                        _ => {}
                    }
                }

                // Implement as a simple conditional for now
                if let (Some(cond), Some(base_expr), Some(_step_expr)) = (condition, base, step) {
                    let cond_val =
                        self.compile_expr(cond, arena, expr_types, locals, function, loop_ctx)?;
                    let cond_bool = cond_val.into_int_value();

                    let then_bb = self.append_block(function, "recurse_base");
                    let else_bb = self.append_block(function, "recurse_step");
                    let merge_bb = self.append_block(function, "recurse_merge");

                    self.cond_br(cond_bool, then_bb, else_bb);

                    self.position_at_end(then_bb);
                    let base_val =
                        self.compile_expr(base_expr, arena, expr_types, locals, function, loop_ctx);
                    let then_exit = self.current_block()?;
                    self.br(merge_bb);

                    self.position_at_end(else_bb);
                    // For step, would need to call self - for now return default
                    // IMPORTANT: Use the same LLVM type as base_val to avoid phi type mismatch
                    let step_val = if let Some(bv) = &base_val {
                        self.cx().default_value_for_type(bv.get_type())
                    } else {
                        self.cx().default_value(result_type)
                    };
                    let else_exit = self.current_block()?;
                    self.br(merge_bb);

                    self.position_at_end(merge_bb);

                    if let Some(bv) = base_val {
                        self.build_phi_from_incoming(
                            result_type,
                            &[(bv, then_exit), (step_val, else_exit)],
                        )
                    } else {
                        Some(step_val)
                    }
                } else {
                    None
                }
            }

            FunctionExpKind::Print => {
                // Find msg parameter and call runtime print function
                for ne in named_exprs {
                    let name = self.cx().interner.lookup(ne.name);
                    if name == "msg" {
                        // Compile the message expression
                        if let Some(msg) = self
                            .compile_expr(ne.value, arena, expr_types, locals, function, loop_ctx)
                        {
                            // Get the ori_print function
                            if let Some(print_fn) = self.cx().llmod().get_function("ori_print") {
                                // Allocate stack space for the string struct
                                let str_type = self.cx().string_type();
                                let msg_ptr = self.alloca(str_type.into(), "print_msg");

                                // Store the string value
                                self.store(msg, msg_ptr);

                                // Call ori_print with the pointer
                                self.call(print_fn, &[msg_ptr.into()], "print_call");
                            }
                        }
                        break;
                    }
                }
                None // print returns void
            }

            FunctionExpKind::Panic => {
                // Call runtime panic function with message, then unreachable
                for ne in named_exprs {
                    let name = self.cx().interner.lookup(ne.name);
                    if name == "msg" {
                        if let Some(msg) = self
                            .compile_expr(ne.value, arena, expr_types, locals, function, loop_ctx)
                        {
                            if let Some(panic_fn) = self.cx().llmod().get_function("ori_panic") {
                                let str_type = self.cx().string_type();
                                let msg_ptr = self.alloca(str_type.into(), "panic_msg");
                                self.store(msg, msg_ptr);
                                self.call(panic_fn, &[msg_ptr.into()], "panic_call");
                            }
                        }
                        break;
                    }
                }
                self.unreachable();
                None
            }

            FunctionExpKind::Todo | FunctionExpKind::Unreachable => {
                // Call runtime panic with static message, then unreachable
                if let Some(panic_fn) = self.cx().llmod().get_function("ori_panic_cstr") {
                    let msg = match exp.kind {
                        FunctionExpKind::Todo => "not yet implemented",
                        FunctionExpKind::Unreachable => "entered unreachable code",
                        _ => "panic",
                    };
                    let msg_ptr = self.build_global_string_ptr(msg, "panic_msg");
                    self.call(panic_fn, &[msg_ptr.into()], "panic_call");
                }
                self.unreachable();
                None
            }

            FunctionExpKind::Catch => {
                // TODO: Implement catch pattern for panic recovery
                // For now, just compile the inner expression and return default
                tracing::debug!("catch pattern not yet implemented in LLVM backend");
                self.default_for_result_type(result_type)
            }

            // Concurrency patterns - not yet implemented in LLVM backend
            FunctionExpKind::Parallel
            | FunctionExpKind::Spawn
            | FunctionExpKind::Timeout
            | FunctionExpKind::Cache
            | FunctionExpKind::With => {
                tracing::debug!(
                    pattern = %exp.kind.name(),
                    "concurrency/capability pattern not yet implemented in LLVM backend"
                );
                self.default_for_result_type(result_type)
            }
        }
    }

    /// Return the default value for a result type, handling void specially.
    fn default_for_result_type(&self, result_type: TypeId) -> Option<BasicValueEnum<'ll>> {
        if result_type == TypeId::VOID {
            None
        } else {
            Some(self.cx().default_value(result_type))
        }
    }
}
