//! Option and Result wrapper compilation (Some, None, Ok, Err).

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, Name, TypeId};
use tracing::instrument;

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile Some(value).
    ///
    /// Uses standardized { i8 tag, i64 payload } layout to match function signatures.
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_some(
        &self,
        inner: ExprId,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the inner value
        let inner_val = self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?;

        // Use standardized Option type with i64 payload
        let opt_type = self.cx().option_type(self.cx().scx.type_i64().into());

        // Coerce inner value to i64 for storage
        let payload = self.coerce_to_i64(inner_val)?;

        // Build the struct: { tag = 1, value = payload }
        let tag = self.cx().scx.type_i8().const_int(1, false); // 1 = Some

        let struct_val = self.build_struct(opt_type, &[tag.into(), payload.into()], "some");

        Some(struct_val.into())
    }

    /// Compile None.
    pub(crate) fn compile_none(&self, type_id: TypeId) -> Option<BasicValueEnum<'ll>> {
        // For None, we need to know the inner type to create the right struct.
        // Since we don't have that info easily, use i64 as default payload.
        let payload_type = self.cx().llvm_type(type_id);

        // If we got a pointer type (unknown), use i64 as default
        let payload_type = if payload_type.is_pointer_type() {
            self.cx().scx.type_i64().into()
        } else {
            payload_type
        };

        let opt_type = self.cx().option_type(payload_type);

        // Build the struct: { tag = 0, value = undef }
        let tag = self.cx().scx.type_i8().const_int(0, false); // 0 = None
        let default_val = self.cx().default_value_for_type(payload_type);

        let struct_val = self.build_struct(opt_type, &[tag.into(), default_val], "none");

        Some(struct_val.into())
    }

    /// Compile Ok(value).
    ///
    /// Uses standardized { i8 tag, i64 payload } layout to match function signatures.
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_ok(
        &self,
        inner: Option<ExprId>,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get the inner value (or use unit if None)
        let inner_val = if let Some(inner_id) = inner {
            self.compile_expr(inner_id, arena, expr_types, locals, function, loop_ctx)?
        } else {
            // Ok() with no value - use a dummy i64
            self.cx().scx.type_i64().const_int(0, false).into()
        };

        // Use standardized Result type with i64 payload for consistent ABI
        let result_type = self.cx().result_type(self.cx().scx.type_i64().into());

        // Coerce inner value to i64 for storage
        let payload = self.coerce_to_i64(inner_val)?;

        // Build the struct: { tag = 0, value = payload }
        let tag = self.cx().scx.type_i8().const_int(0, false); // 0 = Ok

        let struct_val = self.build_struct(result_type, &[tag.into(), payload.into()], "ok");

        Some(struct_val.into())
    }

    /// Compile Err(value).
    ///
    /// Uses standardized { i8 tag, i64 payload } layout to match function signatures.
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_err(
        &self,
        inner: Option<ExprId>,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get the inner value (or use unit if None)
        let inner_val = if let Some(inner_id) = inner {
            self.compile_expr(inner_id, arena, expr_types, locals, function, loop_ctx)?
        } else {
            // Err() with no value - use a dummy i64
            self.cx().scx.type_i64().const_int(0, false).into()
        };

        // Use standardized Result type with i64 payload for consistent ABI
        let result_type = self.cx().result_type(self.cx().scx.type_i64().into());

        // Coerce inner value to i64 for storage
        let payload = self.coerce_to_i64(inner_val)?;

        // Build the struct: { tag = 1, value = payload }
        let tag = self.cx().scx.type_i8().const_int(1, false); // 1 = Err

        let struct_val = self.build_struct(result_type, &[tag.into(), payload.into()], "err");

        Some(struct_val.into())
    }
}
