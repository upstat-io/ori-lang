//! Iterable abstraction over lists and ranges for pattern evaluation:
//! [`Iterable`] and [`IterableIter`].

use crate::errors::{ControlAction, EvalError, EvalResult};
use crate::executor::PatternExecutor;
use crate::value::{ListData, RangeValue, Value};

/// Represents something that can be iterated over in pattern evaluation.
///
/// This abstraction unifies the handling of lists and ranges across patterns
/// like `map`, `filter`, `fold`, `find`, and `collect`.
#[derive(Clone)]
pub enum Iterable {
    /// A list of values.
    List(ListData),
    /// A range of integers.
    Range(RangeValue),
}

/// Iterator over Iterable values.
///
/// Uses enum dispatch instead of `Box<dyn Iterator>` for better performance
/// (no heap allocation, no vtable indirection).
pub enum IterableIter<'a> {
    /// Iterator over list elements (cloned).
    List(std::iter::Cloned<std::slice::Iter<'a, Value>>),
    /// Iterator over range integers converted to Values.
    Range(std::iter::Map<std::ops::Range<i64>, fn(i64) -> Value>),
}

impl Iterator for IterableIter<'_> {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::List(iter) => iter.next(),
            Self::Range(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::List(iter) => iter.size_hint(),
            Self::Range(iter) => iter.size_hint(),
        }
    }
}

impl Iterable {
    /// Try to convert a Value into an Iterable.
    ///
    /// Returns an error if the value is neither a list nor a range.
    /// Unbounded ranges are rejected since patterns require finite iteration.
    pub fn try_from_value(value: Value) -> Result<Self, EvalError> {
        match value {
            Value::List(list) => Ok(Iterable::List(list)),
            Value::Range(range) => {
                if range.is_unbounded() {
                    return Err(EvalError::new(
                        "cannot use unbounded range in pattern context (use .iter().take() instead)",
                    ));
                }
                Ok(Iterable::Range(range))
            }
            _ => Err(EvalError::new(format!(
                "expected a list or range, got {}",
                value.type_name()
            ))),
        }
    }

    /// Iterate over the values in this iterable.
    ///
    /// Returns owned `Value`s: cloned from List elements or created from Range integers.
    ///
    /// # Performance
    /// Uses `IterableIter` enum dispatch instead of `Box<dyn Iterator>` for
    /// zero-allocation iteration (no heap allocation, no vtable indirection).
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    fn iter_values(&self) -> IterableIter<'_> {
        match self {
            Iterable::List(list) => IterableIter::List(list.iter().cloned()),
            Iterable::Range(range) => {
                // Unbounded ranges are rejected in try_from_value, so end is always Some.
                // Use unwrap_or(0) as a fallback — unreachable in practice.
                let end_val = range.end.unwrap_or(range.start);
                let end = if range.inclusive {
                    end_val + 1
                } else {
                    end_val
                };
                IterableIter::Range((range.start..end).map(Value::int as fn(i64) -> Value))
            }
        }
    }

    /// Apply a mapping function to each element and collect results into a list.
    pub fn map_values(&self, func: &Value, exec: &mut dyn PatternExecutor) -> EvalResult {
        let mut results = Vec::new();
        for item in self.iter_values() {
            let result = exec.call(func, vec![item])?;
            results.push(result);
        }
        Ok(Value::list(results))
    }

    /// Filter elements using a predicate function and collect results into a list.
    pub fn filter_values(&self, func: &Value, exec: &mut dyn PatternExecutor) -> EvalResult {
        let mut results = Vec::new();
        for item in self.iter_values() {
            let keep = exec.call(func, vec![item.clone()])?;
            if keep.is_truthy() {
                results.push(item);
            }
        }
        Ok(Value::list(results))
    }

    /// Reduce the iterable to a single value using an accumulator function.
    pub fn fold_values(
        &self,
        mut acc: Value,
        func: &Value,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        for item in self.iter_values() {
            acc = exec.call(func, vec![acc, item])?;
        }
        Ok(acc)
    }

    /// Find the first element matching a predicate.
    ///
    /// Returns `Some(value)` if found, `None` if not found.
    pub fn find_value(
        &self,
        func: &Value,
        exec: &mut dyn PatternExecutor,
    ) -> Result<Option<Value>, ControlAction> {
        for item in self.iter_values() {
            let matches = exec.call(func, vec![item.clone()])?;
            if matches.is_truthy() {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }
}
