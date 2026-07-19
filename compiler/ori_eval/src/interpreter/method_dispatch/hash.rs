//! Interpreter-aware `Hashable.hash` dispatch for compound builtin values.

use crate::errors::wrong_function_args;
use crate::methods::compare::hash_combine;
use crate::methods::{dispatch_builtin_method, DispatchCtx};
use crate::{ControlAction, EvalResult, Value};

use super::super::Interpreter;

impl Interpreter<'_> {
    /// Invoke the value's actual `Hashable.hash` implementation.
    pub(in crate::interpreter) fn eval_hashable_value(
        &mut self,
        value: &Value,
    ) -> Result<i64, ControlAction> {
        let result =
            self.eval_method_call(value.clone(), self.builtin_method_names.hash, Vec::new())?;
        let Value::Int(hash) = result else {
            return Err(
                crate::errors::wrong_arg_type("hash", "Hashable method returning int").into(),
            );
        };
        Ok(hash.raw())
    }

    /// Hash a builtin value while preserving dynamic dispatch for nested values.
    pub(super) fn eval_builtin_hash(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if !args.is_empty() {
            return Err(wrong_function_args(0, args.len()).into());
        }

        let hash = match receiver {
            Value::List(values) => self.fold_hashable_values(values.iter())?,
            Value::Tuple(values) => self.fold_hashable_values(values.iter())?,
            Value::None => 0,
            Value::Some(value) => hash_combine(1, self.eval_hashable_value(&value)?),
            Value::Ok(value) => hash_combine(2, self.eval_hashable_value(&value)?),
            Value::Err(value) => hash_combine(3, self.eval_hashable_value(&value)?),
            Value::Map(map) => {
                let mut hash = 0_i64;
                for (key, value) in map.iter() {
                    let key_hash = self.eval_hashable_value(key)?;
                    let value_hash = self.eval_hashable_value(value)?;
                    hash ^= hash_combine(key_hash, value_hash);
                }
                hash
            }
            Value::Set(values) => {
                let mut hash = 0_i64;
                for value in values.values() {
                    hash ^= self.eval_hashable_value(value)?;
                }
                hash
            }
            value => {
                let ctx = DispatchCtx {
                    names: &self.builtin_method_names,
                    interner: self.interner,
                };
                return dispatch_builtin_method(
                    value,
                    self.builtin_method_names.hash,
                    Vec::new(),
                    &ctx,
                );
            }
        };

        Ok(Value::int(hash))
    }

    fn fold_hashable_values<'value>(
        &mut self,
        values: impl Iterator<Item = &'value Value>,
    ) -> Result<i64, ControlAction> {
        let mut hash = 0_i64;
        for value in values {
            hash = hash_combine(hash, self.eval_hashable_value(value)?);
        }
        Ok(hash)
    }
}
