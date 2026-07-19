//! Method argument count, type, and index validation.

use ori_patterns::{wrong_arg_count, wrong_arg_type, EvalError, ScalarInt, Value};

#[inline]
pub(crate) fn require_args(method: &str, expected: usize, actual: usize) -> Result<(), EvalError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wrong_arg_count(method, expected, actual))
    }
}

macro_rules! value_arg {
    ($name:ident, $output:ty, $pattern:pat => $value:expr, $expected:literal) => {
        #[inline]
        pub(crate) fn $name(
            method: &str,
            args: &[Value],
            index: usize,
        ) -> Result<$output, EvalError> {
            match args.get(index) {
                Some($pattern) => Ok($value),
                _ => Err(wrong_arg_type(method, $expected)),
            }
        }
    };
}

#[inline]
pub(crate) fn require_str_arg<'a>(
    method: &str,
    args: &'a [Value],
    index: usize,
) -> Result<&'a str, EvalError> {
    match args.get(index) {
        Some(Value::Str(value)) => Ok(value),
        _ => Err(wrong_arg_type(method, "string")),
    }
}

#[inline]
pub(crate) fn require_list_arg<'a>(
    method: &str,
    args: &'a [Value],
    index: usize,
) -> Result<&'a [Value], EvalError> {
    match args.get(index) {
        Some(Value::List(value)) => Ok(value),
        _ => Err(wrong_arg_type(method, "list")),
    }
}

value_arg!(require_int_arg, i64, Value::Int(value) => value.raw(), "int");
value_arg!(require_scalar_int_arg, ScalarInt, Value::Int(value) => *value, "int");
value_arg!(require_float_arg, f64, Value::Float(value) => *value, "float");
value_arg!(require_duration_arg, i64, Value::Duration(value) => *value, "Duration");
value_arg!(require_size_arg, u64, Value::Size(value) => *value, "Size");
value_arg!(require_bool_arg, bool, Value::Bool(value) => *value, "bool");
value_arg!(require_char_arg, char, Value::Char(value) => *value, "char");
value_arg!(require_byte_arg, u8, Value::Byte(value) => *value, "byte");

#[inline]
pub(crate) fn nonnegative_usize(
    value: i64,
    method: &str,
    expected: &'static str,
) -> Result<usize, EvalError> {
    if value < 0 {
        return Err(wrong_arg_type(method, expected));
    }
    usize::try_from(value).map_err(|_| wrong_arg_type(method, expected))
}
