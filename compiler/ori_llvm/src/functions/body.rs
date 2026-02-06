//! Function body compilation.

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, Name};
use ori_types::Idx;
use tracing::{instrument, trace, warn};

use crate::builder::{Builder, Locals};

/// Configuration for compiling a function body.
///
/// Groups the parameters needed for `compile_function_body` into a single struct
/// to reduce parameter count and improve readability.
pub struct FunctionBodyConfig<'a, 'll> {
    /// Parameter names for the function.
    pub param_names: &'a [Name],
    /// The return type of the function.
    pub return_type: Idx,
    /// The expression ID of the function body.
    pub body: ExprId,
    /// The expression arena containing the body expressions.
    pub arena: &'a ExprArena,
    /// Expression type annotations.
    pub expr_types: &'a [Idx],
    /// The LLVM function value to compile into.
    pub function: FunctionValue<'ll>,
    /// Whether this function uses the sret calling convention.
    ///
    /// When true, the original struct return is passed via a hidden first
    /// parameter (param 0). User-visible parameters start at param 1, and
    /// the function returns void after storing the result through the sret pointer.
    pub uses_sret: bool,
}

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a function body after declaration.
    ///
    /// This assumes the function has already been declared and we just need to
    /// compile its body. The builder should be positioned at the entry block.
    #[instrument(skip(self, config), level = "debug")]
    pub fn compile_function_body(&self, config: &FunctionBodyConfig<'_, 'll>) {
        let FunctionBodyConfig {
            param_names,
            return_type,
            body,
            arena,
            expr_types,
            function,
            uses_sret,
        } = *config;

        // When sret is active, param 0 is the hidden sret pointer — user
        // parameters start at index 1.
        let param_offset: u32 = u32::from(uses_sret);

        // Build parameter map (function parameters are immutable)
        let mut locals = Locals::new();

        // Verify parameter count matches (debug assertion for internal consistency).
        // LLVM param count includes the hidden sret pointer when active.
        debug_assert_eq!(
            function.count_params() as usize,
            param_names.len() + param_offset as usize,
            "Function parameter count mismatch: LLVM function has {} params, expected {} (sret={})",
            function.count_params(),
            param_names.len() + param_offset as usize,
            uses_sret
        );

        for (i, &param_name) in param_names.iter().enumerate() {
            let param_value = function
                .get_nth_param(i as u32 + param_offset)
                .expect("internal error: parameter count verified but get_nth_param failed");
            let name_str = self.cx().interner.lookup(param_name);
            param_value.set_name(name_str);
            trace!(param = name_str, idx = i, param_type = ?param_value.get_type(), "binding parameter");
            locals.bind_immutable(param_name, param_value);
        }

        // Compile body (no loop context at top level)
        let result = self.compile_expr(body, arena, expr_types, &mut locals, function, None);

        // If the body already terminated (e.g., panic, unreachable, infinite loop),
        // don't add another terminator - the block is already complete.
        if let Some(block) = self.current_block() {
            if block.get_terminator().is_some() {
                return;
            }
        }

        // sret return path: store result through the hidden sret pointer, then ret void
        if uses_sret {
            if let Some(val) = result {
                let sret_ptr = function
                    .get_nth_param(0)
                    .expect("sret function must have param 0")
                    .into_pointer_value();
                self.store(val, sret_ptr);
            }
            self.ret_void();
            return;
        }

        // Get the function's declared return type from LLVM
        let fn_ret_type = function.get_type().get_return_type();

        // Return
        if return_type == Idx::UNIT {
            self.ret_void();
        } else if let (Some(val), Some(expected_type)) = (result, fn_ret_type) {
            // Check if the result type matches the declared return type
            let actual_type = val.get_type();

            if actual_type == expected_type {
                // Types match, return directly
                trace!(ret_type = ?actual_type, "returning value (types match)");
                self.ret(val);
            } else {
                // Type mismatch - coerce the value to match the declared return type
                warn!(
                    actual = ?actual_type,
                    expected = ?expected_type,
                    "return value type mismatch, coercing"
                );
                let coerced = self.coerce_return_value(val, expected_type);
                self.ret(coerced);
            }
        } else {
            // Fallback: return default value matching the LLVM function's declared return type
            // We use the LLVM type (not Idx) to ensure the value matches the declaration
            if let Some(llvm_ret_type) = fn_ret_type {
                let default = self.cx().default_value_for_type(llvm_ret_type);
                self.ret(default);
            } else {
                self.ret_void();
            }
        }
    }

    /// Coerce a return value to match the function's declared return type.
    pub(crate) fn coerce_return_value(
        &self,
        val: BasicValueEnum<'ll>,
        target_type: inkwell::types::BasicTypeEnum<'ll>,
    ) -> BasicValueEnum<'ll> {
        use inkwell::types::BasicTypeEnum;

        let val_type = val.get_type();

        // If types match, no coercion needed
        if val_type == target_type {
            return val;
        }

        // Struct to int coercion (e.g., Result/Option -> i64)
        // Extract the payload (field 1) from tagged unions
        if let BasicValueEnum::StructValue(sv) = val {
            if let BasicTypeEnum::IntType(target_int) = target_type {
                // If the struct has 2 fields (tag, payload), extract payload
                if sv.get_type().count_fields() == 2 {
                    if let Some(payload) = self.extract_value(sv, 1, "ret_payload") {
                        // If payload is already the right type, return it
                        if payload.get_type() == target_type {
                            return payload;
                        }
                        // If payload is int, extend/truncate to match
                        if let BasicValueEnum::IntValue(iv) = payload {
                            let payload_width = iv.get_type().get_bit_width();
                            let target_width = target_int.get_bit_width();
                            return match payload_width.cmp(&target_width) {
                                std::cmp::Ordering::Less => {
                                    self.zext(iv, target_int, "ret_zext").into()
                                }
                                std::cmp::Ordering::Greater => {
                                    self.trunc(iv, target_int, "ret_trunc").into()
                                }
                                std::cmp::Ordering::Equal => payload,
                            };
                        }
                    }
                }
            }
        }

        // Int to int coercion
        if let (BasicValueEnum::IntValue(iv), BasicTypeEnum::IntType(target_int)) =
            (val, target_type)
        {
            let val_width = iv.get_type().get_bit_width();
            let target_width = target_int.get_bit_width();
            return match val_width.cmp(&target_width) {
                std::cmp::Ordering::Less => self.zext(iv, target_int, "ret_zext").into(),
                std::cmp::Ordering::Greater => self.trunc(iv, target_int, "ret_trunc").into(),
                std::cmp::Ordering::Equal => val,
            };
        }

        // Fallback: return the value as-is (may cause LLVM errors, but better than nothing)
        val
    }
}
