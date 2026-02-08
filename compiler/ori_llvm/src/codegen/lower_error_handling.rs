//! Error handling lowering for V2 codegen.
//!
//! Handles `Ok`, `Err`, `Some`, `None`, and the Try operator (`?`).
//!
//! # Tag Semantics
//!
//! - **Option**: `None=0`, `Some=1`
//! - **Result**: `Ok=0`, `Err=1`
//!
//! # Type Layout (via `TypeLayoutResolver`)
//!
//! - **Option[T]**: `{i8 tag, resolve(T) payload}` — payload matches inner type exactly.
//! - **Result[T, E]**: `{i8 tag, max(resolve(T), resolve(E)) payload}` — payload
//!   is the larger of the two types. Values smaller than the payload slot are
//!   stored via alloca+store+load reinterpretation.

use ori_ir::ExprId;
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::type_info::TypeInfo;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Option constructors
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Some(inner)` → `{i8 tag=1, <inner_type> payload}`.
    ///
    /// With `TypeLayoutResolver`, the Option struct is `{i8, resolve(T)}`.
    /// The inner value's type matches the payload slot exactly — no coercion needed.
    pub(crate) fn lower_some(&mut self, inner: ExprId, expr_id: ExprId) -> Option<ValueId> {
        let result_type = self.expr_type(expr_id);
        let opt_ty = self.resolve_type(result_type);
        let tag = self.builder.const_i8(1); // Some = 1

        let payload = if inner.is_valid() {
            self.lower(inner)?
        } else {
            // Some(()) — unit payload: produce zero of the inner type
            self.zero_value_for_option_payload(result_type)
        };

        Some(self.builder.build_struct(opt_ty, &[tag, payload], "some"))
    }

    /// Lower `ExprKind::None` → `{i8 tag=0, <zero> payload}`.
    pub(crate) fn lower_none(&mut self, expr_id: ExprId) -> Option<ValueId> {
        let result_type = self.expr_type(expr_id);
        let opt_ty = self.resolve_type(result_type);
        let tag = self.builder.const_i8(0); // None = 0
        let zero_payload = self.zero_value_for_option_payload(result_type);

        Some(
            self.builder
                .build_struct(opt_ty, &[tag, zero_payload], "none"),
        )
    }

    // -----------------------------------------------------------------------
    // Result constructors
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Ok(inner)` → `{i8 tag=0, <payload_type> payload}`.
    ///
    /// The Result struct is `{i8, max(resolve(ok), resolve(err))}`. If the ok
    /// value's type differs from the payload type, we reinterpret via alloca.
    pub(crate) fn lower_ok(&mut self, inner: ExprId, expr_id: ExprId) -> Option<ValueId> {
        let result_type = self.expr_type(expr_id);
        let res_ty = self.resolve_type(result_type);
        let tag = self.builder.const_i8(0); // Ok = 0

        let payload = if inner.is_valid() {
            let val = self.lower(inner)?;
            self.coerce_for_result_payload(val, result_type, true)
        } else {
            self.zero_value_for_result_payload(result_type)
        };

        Some(self.builder.build_struct(res_ty, &[tag, payload], "ok"))
    }

    /// Lower `ExprKind::Err(inner)` → `{i8 tag=1, <payload_type> payload}`.
    pub(crate) fn lower_err(&mut self, inner: ExprId, expr_id: ExprId) -> Option<ValueId> {
        let result_type = self.expr_type(expr_id);
        let res_ty = self.resolve_type(result_type);
        let tag = self.builder.const_i8(1); // Err = 1

        let payload = if inner.is_valid() {
            let val = self.lower(inner)?;
            self.coerce_for_result_payload(val, result_type, false)
        } else {
            self.zero_value_for_result_payload(result_type)
        };

        Some(self.builder.build_struct(res_ty, &[tag, payload], "err"))
    }

    // -----------------------------------------------------------------------
    // Try operator (?)
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Try(inner)` — `expr?` error propagation.
    ///
    /// For Option:
    /// ```text
    ///   %opt = lower(inner)
    ///   %tag = extractvalue %opt, 0
    ///   %is_some = icmp ne %tag, 0
    ///   cond_br %is_some, unwrap_bb, propagate_bb
    /// unwrap:
    ///   %payload = extractvalue %opt, 1
    ///   ; coerce payload to inner type
    /// propagate:
    ///   ; return None from the enclosing function
    /// ```
    ///
    /// For Result:
    /// ```text
    ///   %res = lower(inner)
    ///   %tag = extractvalue %res, 0
    ///   %is_ok = icmp eq %tag, 0
    ///   cond_br %is_ok, unwrap_bb, propagate_bb
    /// unwrap:
    ///   %payload = extractvalue %res, 1
    ///   ; coerce to ok type
    /// propagate:
    ///   ; return Err(err_payload) from the enclosing function
    /// ```
    pub(crate) fn lower_try(&mut self, inner: ExprId, expr_id: ExprId) -> Option<ValueId> {
        let inner_val = self.lower(inner)?;
        let inner_type = self.expr_type(inner);
        let type_info = self.type_info.get(inner_type);

        let is_option = matches!(type_info, TypeInfo::Option { .. });

        // Extract tag
        let tag = self.builder.extract_value(inner_val, 0, "try.tag")?;
        let zero_tag = self.builder.const_i8(0);

        // Determine "has value" condition
        let has_value = if is_option {
            self.builder.icmp_ne(tag, zero_tag, "try.is_some")
        } else {
            self.builder.icmp_eq(tag, zero_tag, "try.is_ok")
        };

        let unwrap_bb = self
            .builder
            .append_block(self.current_function, "try.unwrap");
        let propagate_bb = self
            .builder
            .append_block(self.current_function, "try.propagate");

        self.builder.cond_br(has_value, unwrap_bb, propagate_bb);

        // Propagate: early return with the error/None value
        self.builder.position_at_end(propagate_bb);
        if is_option {
            // Return None — build {tag=0, zero_payload}
            let none_tag = self.builder.const_i8(0);

            // Get the enclosing function's return type to build the right struct
            let fn_ret_type = self.resolve_function_return_type();
            // Produce a zero payload matching the function's option type
            let zero_payload = self.zero_value_for_fn_return_payload(fn_ret_type);
            let ret_val =
                self.builder
                    .build_struct(fn_ret_type, &[none_tag, zero_payload], "try.none");
            self.builder.ret(ret_val);
        } else {
            // Return Err(err_payload) — preserve the error
            let err_payload = self
                .builder
                .extract_value(inner_val, 1, "try.err_payload")
                .unwrap_or_else(|| self.builder.const_i64(0));
            let err_tag = self.builder.const_i8(1);

            let fn_ret_type = self.resolve_function_return_type();
            let ret_val =
                self.builder
                    .build_struct(fn_ret_type, &[err_tag, err_payload], "try.err");
            self.builder.ret(ret_val);
        }

        // Unwrap: extract the success payload
        self.builder.position_at_end(unwrap_bb);
        let payload = self.builder.extract_value(inner_val, 1, "try.payload")?;

        // Coerce payload to the expected type
        let result_type = self.expr_type(expr_id);
        let coerced = self.coerce_payload(payload, result_type);
        Some(coerced)
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Resolve the LLVM type of the enclosing function's return value.
    ///
    /// Used by Try to construct the early-return value with the correct type.
    fn resolve_function_return_type(&mut self) -> super::value_id::LLVMTypeId {
        let func_val = self.builder.get_function_value(self.current_function);
        let ret_ty = func_val.get_type().get_return_type();
        if let Some(ty) = ret_ty {
            self.builder.register_type(ty)
        } else {
            // Void return — shouldn't happen with Try, but fall back
            let i8_tag = self.builder.scx().type_i8().into();
            let i64_payload = self.builder.scx().type_i64().into();
            let struct_ty = self
                .builder
                .scx()
                .type_struct(&[i8_tag, i64_payload], false);
            self.builder.register_type(struct_ty.into())
        }
    }

    /// Produce a zero payload matching the inner type of an Option.
    ///
    /// For `option[int]` → `i64 0`, for `option[bool]` → `i1 0`,
    /// for `option[str]` → `{i64 0, ptr null}`, etc.
    fn zero_value_for_option_payload(&mut self, option_idx: Idx) -> ValueId {
        let type_info = self.type_info.get(option_idx);
        match type_info {
            TypeInfo::Option { inner } => {
                let inner_ty = self.type_resolver.resolve(inner);
                self.builder.const_zero(inner_ty)
            }
            // Not an Option — fall back to i64 zero (shouldn't happen)
            _ => self.builder.const_i64(0),
        }
    }

    /// Produce a zero payload for a Result's payload slot.
    ///
    /// The payload type is `max(resolve(ok), resolve(err))`.
    fn zero_value_for_result_payload(&mut self, result_idx: Idx) -> ValueId {
        let payload_ty = self.resolve_result_payload_type(result_idx);
        self.builder.const_zero(payload_ty)
    }

    /// Get the LLVM type for a Result's payload slot.
    ///
    /// Returns `max(resolve(ok), resolve(err))` — the larger of the two types.
    fn resolve_result_payload_type(&self, result_idx: Idx) -> inkwell::types::BasicTypeEnum<'ctx> {
        let type_info = self.type_info.get(result_idx);
        match type_info {
            TypeInfo::Result { ok, err } => {
                let ok_ty = self.type_resolver.resolve(ok);
                let err_ty = self.type_resolver.resolve(err);
                let ok_size = super::type_info::TypeLayoutResolver::type_store_size(ok_ty);
                let err_size = super::type_info::TypeLayoutResolver::type_store_size(err_ty);
                if ok_size >= err_size {
                    ok_ty
                } else {
                    err_ty
                }
            }
            // Not a Result — fall back to i64
            _ => self.builder.scx().type_i64().into(),
        }
    }

    /// Coerce a value for storage in a Result's payload slot.
    ///
    /// If the value's type matches the payload type, use it directly.
    /// Otherwise, store the value through a payload-sized alloca and load
    /// back as the payload type (reinterpretation cast).
    fn coerce_for_result_payload(
        &mut self,
        val: ValueId,
        result_idx: Idx,
        _is_ok: bool,
    ) -> ValueId {
        let payload_ty = self.resolve_result_payload_type(result_idx);
        let val_raw = self.builder.raw_value(val);

        // If types already match, no coercion needed (common case)
        if val_raw.get_type() == payload_ty {
            return val;
        }

        // Reinterpret via alloca: allocate payload-sized slot, store value,
        // load as payload type. This handles size mismatches like storing
        // bool (i1) into an i64-sized payload slot.
        let payload_ty_id = self.builder.register_type(payload_ty);
        let ptr = self.builder.create_entry_alloca(
            self.current_function,
            "result.payload.cast",
            payload_ty_id,
        );
        // Zero-initialize the alloca to avoid undefined bits in padding
        let zero = self.builder.const_zero(payload_ty);
        self.builder.store(zero, ptr);
        // Store the actual value (smaller type writes to the beginning)
        self.builder.store(val, ptr);
        self.builder.load(payload_ty_id, ptr, "result.payload")
    }

    /// Produce a zero payload for the function's return type (used by Try propagation).
    ///
    /// Extracts the payload type from the struct type (field 1) and produces
    /// a zero value of that type.
    fn zero_value_for_fn_return_payload(
        &mut self,
        fn_ret_type: super::value_id::LLVMTypeId,
    ) -> ValueId {
        let raw_ty = self.builder.raw_type(fn_ret_type);
        if let inkwell::types::BasicTypeEnum::StructType(st) = raw_ty {
            if let Some(field_ty) = st.get_field_type_at_index(1) {
                return self.builder.const_zero(field_ty);
            }
        }
        // Fallback: i64 zero
        self.builder.const_i64(0)
    }
}
