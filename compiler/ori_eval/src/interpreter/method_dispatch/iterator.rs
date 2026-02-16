//! Iterator method dispatch for the Interpreter.
//!
//! Handles both adapter constructors (`map`, `filter`, `take`, `skip`) that return
//! new lazy iterators, and consumer methods (`fold`, `count`, `find`, `any`, `all`,
//! `for_each`, `collect`) that eagerly consume iterators.
//!
//! Adapter iterators capture closures and need `eval_call()` to invoke them
//! on each `next()` call, which is why they live at the interpreter level
//! rather than in the pure `IteratorValue::next()`.

use ori_patterns::IteratorValue;

use crate::{ControlAction, EvalError, EvalResult, Value};

use super::super::resolvers::CollectionMethod;
use super::Interpreter;

impl Interpreter<'_> {
    /// Dispatch an iterator method call.
    pub(super) fn eval_iterator_method(
        &mut self,
        receiver: Value,
        method: CollectionMethod,
        args: &[Value],
    ) -> EvalResult {
        let Value::Iterator(iter_val) = receiver else {
            unreachable!("eval_iterator_method called with non-iterator receiver")
        };

        match method {
            CollectionMethod::IterNext => {
                Self::expect_arg_count("next", 0, args)?;
                self.eval_iter_next_as_tuple(iter_val)
            }
            CollectionMethod::IterMap => {
                Self::expect_arg_count("map", 1, args)?;
                Ok(Self::make_mapped(iter_val, args[0].clone()))
            }
            CollectionMethod::IterFilter => {
                Self::expect_arg_count("filter", 1, args)?;
                Ok(Self::make_filtered(iter_val, args[0].clone()))
            }
            CollectionMethod::IterTake => {
                Self::expect_arg_count("take", 1, args)?;
                Self::make_take(iter_val, &args[0])
            }
            CollectionMethod::IterSkip => {
                Self::expect_arg_count("skip", 1, args)?;
                Self::make_skip(iter_val, &args[0])
            }
            CollectionMethod::IterFold => {
                Self::expect_arg_count("fold", 2, args)?;
                self.eval_iter_fold(iter_val, args[0].clone(), &args[1])
            }
            CollectionMethod::IterCount => {
                Self::expect_arg_count("count", 0, args)?;
                self.eval_iter_count(iter_val)
            }
            CollectionMethod::IterFind => {
                Self::expect_arg_count("find", 1, args)?;
                self.eval_iter_find(iter_val, &args[0])
            }
            CollectionMethod::IterAny => {
                Self::expect_arg_count("any", 1, args)?;
                self.eval_iter_any(iter_val, &args[0])
            }
            CollectionMethod::IterAll => {
                Self::expect_arg_count("all", 1, args)?;
                self.eval_iter_all(iter_val, &args[0])
            }
            CollectionMethod::IterForEach => {
                Self::expect_arg_count("for_each", 1, args)?;
                self.eval_iter_for_each(iter_val, &args[0])
            }
            CollectionMethod::IterCollect => {
                Self::expect_arg_count("collect", 0, args)?;
                self.eval_iter_collect(iter_val)
            }
            _ => unreachable!("non-iterator CollectionMethod in eval_iterator_method"),
        }
    }

    // ── Core: advance one step ──────────────────────────────────────────

    /// Advance an iterator by one step, handling both source and adapter variants.
    ///
    /// Returns `(Option<Value>, IteratorValue)` — the yielded item and the
    /// advanced iterator state.
    fn eval_iter_next(
        &mut self,
        iter_val: IteratorValue,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        match iter_val {
            // Source variants — pure, no interpreter needed
            IteratorValue::List { .. }
            | IteratorValue::Range { .. }
            | IteratorValue::Map { .. }
            | IteratorValue::Set { .. }
            | IteratorValue::Str { .. } => {
                let (item, new_iter) = iter_val.next();
                Ok((item, new_iter))
            }

            // Mapped: get next from source, apply transform
            IteratorValue::Mapped { source, transform } => {
                let (item, new_source) = self.eval_iter_next(*source)?;
                match item {
                    Some(val) => {
                        let mapped = self.eval_call(&transform, &[val])?;
                        Ok((
                            Some(mapped),
                            IteratorValue::Mapped {
                                source: Box::new(new_source),
                                transform,
                            },
                        ))
                    }
                    None => Ok((
                        None,
                        IteratorValue::Mapped {
                            source: Box::new(new_source),
                            transform,
                        },
                    )),
                }
            }

            // Filtered: loop source until predicate passes or exhausted
            IteratorValue::Filtered { source, predicate } => {
                let mut current = *source;
                loop {
                    let (item, new_source) = self.eval_iter_next(current)?;
                    match item {
                        Some(val) => {
                            let keep = self.eval_call(&predicate, std::slice::from_ref(&val))?;
                            if keep.is_truthy() {
                                return Ok((
                                    Some(val),
                                    IteratorValue::Filtered {
                                        source: Box::new(new_source),
                                        predicate,
                                    },
                                ));
                            }
                            // Predicate rejected — advance and try again
                            current = new_source;
                        }
                        None => {
                            return Ok((
                                None,
                                IteratorValue::Filtered {
                                    source: Box::new(new_source),
                                    predicate,
                                },
                            ));
                        }
                    }
                }
            }

            // TakeN: yield up to `remaining` items
            IteratorValue::TakeN { source, remaining } => {
                if remaining == 0 {
                    return Ok((
                        None,
                        IteratorValue::TakeN {
                            source,
                            remaining: 0,
                        },
                    ));
                }
                let (item, new_source) = self.eval_iter_next(*source)?;
                let new_remaining = remaining.saturating_sub(1);
                Ok((
                    item,
                    IteratorValue::TakeN {
                        source: Box::new(new_source),
                        remaining: new_remaining,
                    },
                ))
            }

            // SkipN: skip first `remaining` items, then yield normally
            IteratorValue::SkipN { source, remaining } => {
                let mut current = *source;
                for _ in 0..remaining {
                    let (item, new_source): (Option<Value>, IteratorValue) =
                        self.eval_iter_next(current)?;
                    current = new_source;
                    if item.is_none() {
                        // Source exhausted before we finished skipping
                        return Ok((
                            None,
                            IteratorValue::SkipN {
                                source: Box::new(current),
                                remaining: 0,
                            },
                        ));
                    }
                }
                // Done skipping — now yield normally
                let (item, new_source) = self.eval_iter_next(current)?;
                Ok((
                    item,
                    IteratorValue::SkipN {
                        source: Box::new(new_source),
                        remaining: 0,
                    },
                ))
            }
        }
    }

    /// `next()` returns `(T?, Iterator<T>)` tuple for the Ori protocol.
    fn eval_iter_next_as_tuple(&mut self, iter_val: IteratorValue) -> EvalResult {
        let (maybe_item, new_iter) = self.eval_iter_next(iter_val)?;
        let option_val = match maybe_item {
            Some(v) => Value::some(v),
            None => Value::None,
        };
        Ok(Value::tuple(vec![option_val, Value::iterator(new_iter)]))
    }

    // ── Adapter constructors ────────────────────────────────────────────

    /// Create a `Mapped` adapter iterator.
    fn make_mapped(iter_val: IteratorValue, transform: Value) -> Value {
        Value::iterator(IteratorValue::Mapped {
            source: Box::new(iter_val),
            transform: Box::new(transform),
        })
    }

    /// Create a `Filtered` adapter iterator.
    fn make_filtered(iter_val: IteratorValue, predicate: Value) -> Value {
        Value::iterator(IteratorValue::Filtered {
            source: Box::new(iter_val),
            predicate: Box::new(predicate),
        })
    }

    /// Create a `TakeN` adapter iterator.
    fn make_take(iter_val: IteratorValue, count_val: &Value) -> EvalResult {
        let count = Self::extract_usize(count_val, "take")?;
        Ok(Value::iterator(IteratorValue::TakeN {
            source: Box::new(iter_val),
            remaining: count,
        }))
    }

    /// Create a `SkipN` adapter iterator.
    fn make_skip(iter_val: IteratorValue, count_val: &Value) -> EvalResult {
        let count = Self::extract_usize(count_val, "skip")?;
        Ok(Value::iterator(IteratorValue::SkipN {
            source: Box::new(iter_val),
            remaining: count,
        }))
    }

    /// Extract a non-negative integer from a Value for take/skip.
    fn extract_usize(val: &Value, method: &str) -> Result<usize, ControlAction> {
        match val {
            Value::Int(n) => {
                let n = n.raw();
                usize::try_from(n).map_err(|_| {
                    EvalError::new(format!("{method} count must be non-negative, got {n}")).into()
                })
            }
            other => Err(EvalError::new(format!(
                "{method} expects int argument, got {}",
                other.type_name()
            ))
            .into()),
        }
    }

    // ── Consumer methods ────────────────────────────────────────────────

    /// `fold(initial, op)` — accumulate by calling `op(acc, item)` for each item.
    fn eval_iter_fold(
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
    fn eval_iter_count(&mut self, iter_val: IteratorValue) -> EvalResult {
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
    fn eval_iter_find(&mut self, iter_val: IteratorValue, predicate: &Value) -> EvalResult {
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
    fn eval_iter_any(&mut self, iter_val: IteratorValue, predicate: &Value) -> EvalResult {
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
    fn eval_iter_all(&mut self, iter_val: IteratorValue, predicate: &Value) -> EvalResult {
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
    fn eval_iter_for_each(&mut self, iter_val: IteratorValue, f: &Value) -> EvalResult {
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
    fn eval_iter_collect(&mut self, iter_val: IteratorValue) -> EvalResult {
        let mut result = Vec::new();
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
