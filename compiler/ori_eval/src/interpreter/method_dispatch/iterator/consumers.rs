//! Iterator consumer methods that eagerly consume iterators.
//!
//! Consumers drive the iterator to completion: `fold`, `count`, `find`,
//! `any`, `all`, `for_each`, `collect`.

use ori_patterns::IteratorValue;

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
        let mut result = Vec::with_capacity(lower);
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
}
