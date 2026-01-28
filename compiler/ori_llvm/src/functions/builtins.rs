//! Built-in type conversion functions.

use inkwell::values::BasicValueEnum;

use crate::builder::Builder;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile str(x) - convert a value to string.
    pub(crate) fn compile_builtin_str(
        &self,
        val: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        match val.get_type() {
            inkwell::types::BasicTypeEnum::IntType(int_ty) => {
                let bit_width = int_ty.get_bit_width();
                if bit_width == 64 {
                    // Call runtime function to convert i64 to string
                    let func = self.cx().llmod().get_function("ori_str_from_int")?;
                    self.call(func, &[val], "str_from_int")
                } else if bit_width == 1 {
                    // bool -> str
                    let func = self.cx().llmod().get_function("ori_str_from_bool")?;
                    self.call(func, &[val], "str_from_bool")
                } else {
                    // Other int types - zero extend to i64 first
                    let ext = self.zext(val.into_int_value(), self.cx().scx.type_i64(), "zext");
                    let func = self.cx().llmod().get_function("ori_str_from_int")?;
                    self.call(func, &[ext.into()], "str_from_int")
                }
            }
            inkwell::types::BasicTypeEnum::FloatType(_) => {
                // float -> str
                let func = self.cx().llmod().get_function("ori_str_from_float")?;
                self.call(func, &[val], "str_from_float")
            }
            inkwell::types::BasicTypeEnum::StructType(_) => {
                // Assume it's already a string struct
                Some(val)
            }
            _ => {
                // Unknown type - return as-is
                Some(val)
            }
        }
    }

    /// Compile int(x) - convert a value to int.
    pub(crate) fn compile_builtin_int(
        &self,
        val: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        match val.get_type() {
            inkwell::types::BasicTypeEnum::IntType(int_ty) => {
                let bit_width = int_ty.get_bit_width();
                if bit_width == 64 {
                    // Already i64
                    Some(val)
                } else if bit_width == 1 {
                    // bool -> i64 (0 or 1)
                    Some(
                        self.zext(
                            val.into_int_value(),
                            self.cx().scx.type_i64(),
                            "bool_to_int",
                        )
                        .into(),
                    )
                } else {
                    // Other int types - sign extend to i64
                    Some(
                        self.sext(val.into_int_value(), self.cx().scx.type_i64(), "int_ext")
                            .into(),
                    )
                }
            }
            inkwell::types::BasicTypeEnum::FloatType(_) => {
                // float -> int (truncate)
                Some(
                    self.fptosi(
                        val.into_float_value(),
                        self.cx().scx.type_i64(),
                        "float_to_int",
                    )
                    .into(),
                )
            }
            _ => {
                // Unknown type - return 0
                Some(self.cx().scx.type_i64().const_int(0, false).into())
            }
        }
    }

    /// Compile float(x) - convert a value to float.
    pub(crate) fn compile_builtin_float(
        &self,
        val: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        match val.get_type() {
            inkwell::types::BasicTypeEnum::IntType(int_ty) => {
                let bit_width = int_ty.get_bit_width();
                if bit_width == 1 {
                    // bool -> float (0.0 or 1.0)
                    let ext = self.zext(val.into_int_value(), self.cx().scx.type_i64(), "bool_ext");
                    Some(
                        self.sitofp(ext, self.cx().scx.type_f64(), "bool_to_float")
                            .into(),
                    )
                } else {
                    // int -> float
                    Some(
                        self.sitofp(
                            val.into_int_value(),
                            self.cx().scx.type_f64(),
                            "int_to_float",
                        )
                        .into(),
                    )
                }
            }
            inkwell::types::BasicTypeEnum::FloatType(_) => {
                // Already float
                Some(val)
            }
            _ => {
                // Unknown type - return 0.0
                Some(self.cx().scx.type_f64().const_float(0.0).into())
            }
        }
    }

    /// Compile byte(x) - convert a value to byte.
    pub(crate) fn compile_builtin_byte(
        &self,
        val: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        match val.get_type() {
            inkwell::types::BasicTypeEnum::IntType(int_ty) => {
                let bit_width = int_ty.get_bit_width();
                if bit_width == 8 {
                    // Already i8
                    Some(val)
                } else {
                    // Truncate to i8
                    Some(
                        self.trunc(val.into_int_value(), self.cx().scx.type_i8(), "to_byte")
                            .into(),
                    )
                }
            }
            _ => {
                // Unknown type - return 0
                Some(self.cx().scx.type_i8().const_int(0, false).into())
            }
        }
    }
}
