use ori_ir::canon::ConstValue;
use ori_ir::{builtin_constants::size, DurationUnit, SizeUnit, StringInterner};
use ori_patterns::{EvalResult, Value};

use crate::errors::integer_overflow;

pub(super) fn eval_can_duration(value: u64, unit: DurationUnit) -> EvalResult {
    Ok(Value::Duration(
        unit.to_nanos(value)
            .ok_or_else(|| integer_overflow("duration literal"))?,
    ))
}

pub(super) fn eval_can_size(value: u64, unit: SizeUnit) -> EvalResult {
    Ok(Value::Size(
        unit.to_bytes(value)
            .filter(|bytes| *bytes <= size::MAX_BYTES)
            .ok_or_else(|| integer_overflow("size literal"))?,
    ))
}

/// Converts a constant-pool value to its runtime representation.
#[expect(
    clippy::expect_used,
    reason = "Constants come from cooker-validated literals (overflow-checked) or \
              const-fold results (Nanoseconds/Bytes unit, i64-bounded arithmetic). \
              Both paths guarantee to_nanos/to_bytes succeed."
)]
pub(super) fn const_to_value(cv: &ConstValue, interner: &StringInterner) -> Value {
    match *cv {
        ConstValue::Int(n) => Value::int(n),
        ConstValue::Float(bits) => Value::Float(f64::from_bits(bits)),
        ConstValue::Bool(value) => Value::Bool(value),
        ConstValue::Str(name) => Value::string_static(interner.lookup_static(name)),
        ConstValue::Char(value) => Value::Char(value),
        ConstValue::Unit => Value::Void,
        ConstValue::Duration { value, unit } => Value::Duration(
            unit.to_nanos(value)
                .expect("duration overflow: constant should have been validated"),
        ),
        ConstValue::Size { value, unit } => Value::Size(
            unit.to_bytes(value)
                .expect("size overflow: constant should have been validated"),
        ),
    }
}
