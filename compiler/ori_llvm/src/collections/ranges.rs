//! Range compilation.

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId};
use ori_types::Idx;
use tracing::instrument;

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a range expression.
    /// Ranges are represented as { i64 start, i64 end, i1 inclusive }.
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "trace"
    )]
    pub(crate) fn compile_range(
        &self,
        start: ExprId,
        end: ExprId,
        inclusive: bool,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile start (default to 0)
        let start_val = if start.is_present() {
            self.compile_expr(start, arena, expr_types, locals, function, loop_ctx)?
                .into_int_value()
        } else {
            self.cx().scx.type_i64().const_int(0, false)
        };

        // Compile end (default to i64::MAX)
        let end_val = if end.is_present() {
            self.compile_expr(end, arena, expr_types, locals, function, loop_ctx)?
                .into_int_value()
        } else {
            self.cx().scx.type_i64().const_int(i64::MAX as u64, false)
        };

        // Create range struct
        let range_type = self.cx().range_type();
        let inclusive_val = self
            .cx()
            .scx
            .type_i1()
            .const_int(u64::from(inclusive), false);

        let range_val = self.build_struct(
            range_type,
            &[start_val.into(), end_val.into(), inclusive_val.into()],
            "range",
        );

        Some(range_val.into())
    }
}
