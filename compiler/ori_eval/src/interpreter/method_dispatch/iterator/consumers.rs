//! Iterator consumer methods that eagerly consume iterators.
//!
//! Consumers drive the iterator to completion: `fold`, `count`, `find`,
//! `any`, `all`, `for_each`, `collect`.
//!
//! Backward consumers (`last`, `rfind`, `rfold`) use `eval_iter_next_back()`
//! and require double-ended iterators.

use ori_patterns::IteratorValue;

use crate::errors::wrong_arg_type;
use crate::{EvalResult, Value};

use super::super::Interpreter;

impl Interpreter<'_> {
    /// `fold(initial, op)` — accumulate by calling `op(acc, item)` for each item.
    pub(in crate::interpreter) fn eval_iter_fold(
        &mut self,
        iter_val: IteratorValue,
        mut acc: Value,
        op: &Value,
    ) -> EvalResult {
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next(current)?;
            match item {
                Some(val) => {
                    acc = self.eval_call(op, &[acc, val])?;
                    current = new_iter;
                }
                None => return Ok(acc),
            }
        }
    }

    /// `count()` — count items by advancing until exhausted.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "count increments are bounded by iterator length"
    )]
    pub(in crate::interpreter) fn eval_iter_count(
        &mut self,
        iter_val: IteratorValue,
    ) -> EvalResult {
        let mut count: i64 = 0;
        let mut current = iter_val;
        loop {
            let (item, new_iter): (Option<Value>, IteratorValue) = self.eval_iter_next(current)?;
            match item {
                Some(_) => {
                    count += 1;
                    current = new_iter;
                }
                None => return Ok(Value::int(count)),
            }
        }
    }

    /// `find(predicate)` — return first item matching predicate, or None.
    pub(in crate::interpreter) fn eval_iter_find(
        &mut self,
        iter_val: IteratorValue,
        predicate: &Value,
    ) -> EvalResult {
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next(current)?;
            match item {
                Some(val) => {
                    let found = self.eval_call(predicate, std::slice::from_ref(&val))?;
                    if found.is_truthy() {
                        return Ok(Value::some(val));
                    }
                    current = new_iter;
                }
                None => return Ok(Value::None),
            }
        }
    }

    /// `any(predicate)` — short-circuit true if any item matches.
    pub(in crate::interpreter) fn eval_iter_any(
        &mut self,
        iter_val: IteratorValue,
        predicate: &Value,
    ) -> EvalResult {
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next(current)?;
            match item {
                Some(val) => {
                    let result = self.eval_call(predicate, std::slice::from_ref(&val))?;
                    if result.is_truthy() {
                        return Ok(Value::Bool(true));
                    }
                    current = new_iter;
                }
                None => return Ok(Value::Bool(false)),
            }
        }
    }

    /// `all(predicate)` — short-circuit false if any item fails.
    pub(in crate::interpreter) fn eval_iter_all(
        &mut self,
        iter_val: IteratorValue,
        predicate: &Value,
    ) -> EvalResult {
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next(current)?;
            match item {
                Some(val) => {
                    let result = self.eval_call(predicate, std::slice::from_ref(&val))?;
                    if !result.is_truthy() {
                        return Ok(Value::Bool(false));
                    }
                    current = new_iter;
                }
                None => return Ok(Value::Bool(true)),
            }
        }
    }

    /// `for_each(f)` — call `f(item)` for each item, return void.
    pub(in crate::interpreter) fn eval_iter_for_each(
        &mut self,
        iter_val: IteratorValue,
        f: &Value,
    ) -> EvalResult {
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next(current)?;
            match item {
                Some(val) => {
                    self.eval_call(f, &[val])?;
                    current = new_iter;
                }
                None => return Ok(Value::Void),
            }
        }
    }

    /// `collect()` — collect all items into a list.
    pub(in crate::interpreter) fn eval_iter_collect(
        &mut self,
        iter_val: IteratorValue,
    ) -> EvalResult {
        let (lower, _) = iter_val.size_hint();
        // Cap pre-allocation to avoid panic on unbounded iterators (e.g., Repeat, Cycled)
        // where size_hint() returns (usize::MAX, None). The Vec will grow beyond this if needed.
        let mut result = Vec::with_capacity(lower.min(1024 * 1024));
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next(current)?;
            match item {
                Some(val) => {
                    result.push(val);
                    current = new_iter;
                }
                None => return Ok(Value::list(result)),
            }
        }
    }

    /// `__collect_set()` — collect all items into a set (type-directed via Collect trait).
    ///
    /// Rewritten from `collect()` by canonicalization when the expected type is `Set<T>`.
    /// Deduplicates elements using `to_map_key()` as the identity key.
    pub(in crate::interpreter) fn eval_iter_collect_set(
        &mut self,
        iter_val: IteratorValue,
    ) -> EvalResult {
        let mut result = std::collections::BTreeMap::new();
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next(current)?;
            match item {
                Some(val) => {
                    let key = val
                        .to_map_key()
                        .map_err(|e| crate::ControlAction::from(crate::EvalError::new(e)))?;
                    result.entry(key).or_insert(val);
                    current = new_iter;
                }
                None => return Ok(Value::set(result)),
            }
        }
    }

    // ── Backward consumers (require double-ended iterators) ──────────

    /// `last()` — efficiently retrieve the last item via `next_back()`.
    pub(in crate::interpreter) fn eval_iter_last(&mut self, iter_val: IteratorValue) -> EvalResult {
        if !iter_val.is_double_ended() {
            return Err(wrong_arg_type("last", "double-ended iterator").into());
        }
        let (item, _) = self.eval_iter_next_back(iter_val)?;
        match item {
            Some(val) => Ok(Value::some(val)),
            None => Ok(Value::None),
        }
    }

    /// `rfind(predicate)` — find the last item matching predicate via `next_back()`.
    pub(in crate::interpreter) fn eval_iter_rfind(
        &mut self,
        iter_val: IteratorValue,
        predicate: &Value,
    ) -> EvalResult {
        if !iter_val.is_double_ended() {
            return Err(wrong_arg_type("rfind", "double-ended iterator").into());
        }
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next_back(current)?;
            match item {
                Some(val) => {
                    let found = self.eval_call(predicate, std::slice::from_ref(&val))?;
                    if found.is_truthy() {
                        return Ok(Value::some(val));
                    }
                    current = new_iter;
                }
                None => return Ok(Value::None),
            }
        }
    }

    /// `rfold(initial, op)` — accumulate from the back via `next_back()`.
    pub(in crate::interpreter) fn eval_iter_rfold(
        &mut self,
        iter_val: IteratorValue,
        mut acc: Value,
        op: &Value,
    ) -> EvalResult {
        if !iter_val.is_double_ended() {
            return Err(wrong_arg_type("rfold", "double-ended iterator").into());
        }
        let mut current = iter_val;
        loop {
            let (item, new_iter) = self.eval_iter_next_back(current)?;
            match item {
                Some(val) => {
                    acc = self.eval_call(op, &[acc, val])?;
                    current = new_iter;
                }
                None => return Ok(acc),
            }
        }
    }
}
