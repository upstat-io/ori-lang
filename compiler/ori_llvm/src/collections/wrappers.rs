//! Option and Result wrapper compilation (Some, None, Ok, Err).

// Some functions have unused type_id params for API consistency with the trait
#![allow(clippy::used_underscore_binding)]

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId};
use ori_types::Idx;
use tracing::{instrument, trace};

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile Some(value).
    ///
    /// For scalar types (int, float, bool, etc.), uses { i8 tag, i64 payload } layout
    /// with the value coerced to i64. For struct types (nested Options, etc.), uses
    /// { i8 tag, struct payload } to preserve the full struct value.
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_some(
        &self,
        inner: ExprId,
        _type_id: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the inner value
        let inner_val = self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?;

        // Build the struct: { tag = 1, value = payload }
        let tag = self.cx().scx.type_i8().const_int(1, false); // 1 = Some

        // Check if inner value is a struct (e.g., nested Option/Result)
        // If so, store it directly instead of coercing to i64
        if let BasicValueEnum::StructValue(_) = inner_val {
            // Use the actual struct type for the payload
            let inner_type = inner_val.get_type();
            trace!(inner_type = ?inner_type, "Some: struct payload, preserving type");
            let opt_type = self.cx().option_type(inner_type);
            let struct_val = self.build_struct(opt_type, &[tag.into(), inner_val], "some");
            Some(struct_val.into())
        } else {
            trace!(inner_type = ?inner_val.get_type(), "Some: scalar payload, coercing to i64");
            // Use standardized Option type with i64 payload
            let opt_type = self.cx().option_type(self.cx().scx.type_i64().into());

            // Coerce inner value to i64 for storage
            let payload = self.coerce_to_i64(inner_val)?;

            let struct_val = self.build_struct(opt_type, &[tag.into(), payload.into()], "some");

            Some(struct_val.into())
        }
    }

    /// Compile None.
    pub(crate) fn compile_none(&self, type_id: Idx) -> Option<BasicValueEnum<'ll>> {
        // type_id is the type of the whole Option<T> expression.
        // We need to extract the inner type T to build the correct struct.
        let payload_type = if let Some(inner) = self.cx().option_inner_type(type_id) {
            // We know the inner type - use it for the payload
            let llvm_ty = self.cx().llvm_type(inner);
            trace!(?type_id, ?inner, payload_type = ?llvm_ty, "None: resolved inner type");
            llvm_ty
        } else {
            // Fall back to i64 if we can't determine inner type
            trace!(?type_id, "None: unknown inner type, falling back to i64");
            self.cx().scx.type_i64().into()
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
        inner: ExprId,
        _type_id: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get the inner value (or use unit if absent)
        let inner_val = if inner.is_present() {
            self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?
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
        inner: ExprId,
        _type_id: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get the inner value (or use unit if absent)
        let inner_val = if inner.is_present() {
            self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?
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
