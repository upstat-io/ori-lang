//! Built-in type conversion and testing function lowering.
//!
//! Handles `str()`, `int()`, `float()`, `byte()` type conversions and
//! `assert_eq()` test assertions. These are direct calls to named functions
//! that the LLVM backend compiles to concrete runtime calls based on argument type.

use ori_ir::canon::CanRange;
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Lower `str(expr)` — convert value to string.
    pub(crate) fn lower_builtin_str(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        let str_ty = self.resolve_type(Idx::STR);
        let i64_ty = self.builder.i64_type();
        let f64_ty = self.builder.f64_type();
        let bool_ty = self.builder.bool_type();

        match arg_type {
            Idx::INT => {
                let func =
                    self.builder
                        .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty);
                self.builder.call(func, &[val], "str_from_int")
            }
            Idx::FLOAT => {
                let func =
                    self.builder
                        .get_or_declare_function("ori_str_from_float", &[f64_ty], str_ty);
                self.builder.call(func, &[val], "str_from_float")
            }
            Idx::BOOL => {
                let func =
                    self.builder
                        .get_or_declare_function("ori_str_from_bool", &[bool_ty], str_ty);
                self.builder.call(func, &[val], "str_from_bool")
            }
            _ => {
                tracing::warn!(?arg_type, "str() conversion for unsupported type");
                self.builder.record_codegen_error();
                None
            }
        }
    }

    /// Lower `int(expr)` — convert value to int.
    pub(crate) fn lower_builtin_int(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        match arg_type {
            Idx::FLOAT => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.fp_to_si(val, i64_ty, "float2int"))
            }
            Idx::BOOL => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(val, i64_ty, "bool2int"))
            }
            Idx::CHAR => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(val, i64_ty, "char2int"))
            }
            Idx::BYTE => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(val, i64_ty, "byte2int"))
            }
            Idx::INT => Some(val),
            _ => {
                tracing::warn!(?arg_type, "int() conversion for unsupported type");
                Some(val)
            }
        }
    }

    /// Lower `float(expr)` — convert value to float.
    pub(crate) fn lower_builtin_float(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        match arg_type {
            Idx::INT => {
                let f64_ty = self.builder.f64_type();
                Some(self.builder.si_to_fp(val, f64_ty, "int2float"))
            }
            Idx::FLOAT => Some(val),
            _ => {
                tracing::warn!(?arg_type, "float() conversion for unsupported type");
                Some(val)
            }
        }
    }

    /// Lower `byte(expr)` — convert value to byte.
    pub(crate) fn lower_builtin_byte(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        match arg_type {
            Idx::INT => {
                let i8_ty = self.builder.i8_type();
                Some(self.builder.trunc(val, i8_ty, "int2byte"))
            }
            Idx::BYTE => Some(val),
            _ => {
                tracing::warn!(?arg_type, "byte() conversion for unsupported type");
                Some(val)
            }
        }
    }

    /// Lower `assert_eq(actual, expected)` → typed runtime call.
    ///
    /// Generic `assert_eq<T: Eq>` can't be compiled by the LLVM backend (no
    /// monomorphization). Instead, we dispatch to a concrete runtime function
    /// based on the argument type: `ori_assert_eq_int`, `ori_assert_eq_bool`,
    /// `ori_assert_eq_float`, or `ori_assert_eq_str`.
    pub(crate) fn lower_builtin_assert_eq(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let actual_id = *arg_ids.first()?;
        let expected_id = *arg_ids.get(1)?;

        let actual_type = self.expr_type(actual_id);
        let (func_name, pass_by_ptr) = match actual_type {
            Idx::INT => ("ori_assert_eq_int", false),
            Idx::BOOL => ("ori_assert_eq_bool", false),
            Idx::FLOAT => ("ori_assert_eq_float", false),
            Idx::STR => ("ori_assert_eq_str", true),
            _ => {
                tracing::warn!(?actual_type, "assert_eq: unsupported argument type");
                self.builder.record_codegen_error();
                return None;
            }
        };

        let actual = self.lower(actual_id)?;
        let expected = self.lower(expected_id)?;

        let llvm_func = self.builder.scx().llmod.get_function(func_name)?;
        let func_id = self.builder.intern_function(llvm_func);

        if pass_by_ptr {
            // Strings are {i64, ptr} structs — runtime expects pointers
            let actual_ptr = self.alloca_and_store(actual, "assert_eq.actual");
            let expected_ptr = self.alloca_and_store(expected, "assert_eq.expected");
            self.builder.call(func_id, &[actual_ptr, expected_ptr], "")
        } else {
            self.builder.call(func_id, &[actual, expected], "")
        }
    }
}
