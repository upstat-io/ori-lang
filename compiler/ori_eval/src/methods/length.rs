//! Collection-length conversion to evaluator values.

use ori_patterns::{EvalError, EvalResult, Value};

#[inline]
pub(crate) fn len_to_value(len: usize, collection_type: &str) -> EvalResult {
    match i64::try_from(len) {
        Ok(length) => Ok(Value::int(length)),
        Err(error) => Err(EvalError::new(format!("{collection_type} too large: {error}")).into()),
    }
}
