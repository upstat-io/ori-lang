//! Named expression patterns (recurse, parallel, etc.).

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::patterns::FunctionExp;
use ori_ir::{ExprArena, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a FunctionExp (recurse, parallel, etc.).
    pub(crate) fn compile_function_exp(
        &self,
        exp: &FunctionExp,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
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
                    let cond_val = self.compile_expr(cond, arena, expr_types, locals, function, loop_ctx)?;
                    let cond_bool = cond_val.into_int_value();

                    let then_bb = self.append_block(function, "recurse_base");
                    let else_bb = self.append_block(function, "recurse_step");
                    let merge_bb = self.append_block(function, "recurse_merge");

                    self.cond_br(cond_bool, then_bb, else_bb);

                    self.position_at_end(then_bb);
                    let base_val = self.compile_expr(base_expr, arena, expr_types, locals, function, loop_ctx);
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
                        self.build_phi_from_incoming(result_type, &[(bv, then_exit), (step_val, else_exit)])
                    } else {
                        Some(step_val)
                    }
                } else {
                    None
                }
            }

            FunctionExpKind::Print => {
                // Find msg parameter
                for ne in named_exprs {
                    let name = self.cx().interner.lookup(ne.name);
                    if name == "msg" {
                        // Compile the message (but we don't have a runtime print yet)
                        let _msg = self.compile_expr(ne.value, arena, expr_types, locals, function, loop_ctx);
                        // Would call runtime print function here
                    }
                }
                None // print returns void
            }

            FunctionExpKind::Panic => {
                // Compile panic - would call runtime panic function
                // For now, just create unreachable
                self.unreachable();
                None
            }

            _ => {
                // Patterns without custom LLVM codegen — return default for now.
                // Avoids coupling: new FunctionExpKind variants don't require changes here.
                if result_type == TypeId::VOID {
                    None
                } else {
                    Some(self.cx().default_value(result_type))
                }
            }
        }
    }
}
