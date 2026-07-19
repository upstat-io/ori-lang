//! Core iterator advancement logic.
//!
//! `eval_iter_next()` handles all `IteratorValue` variants, routing pure
//! source variants to `IteratorValue::next()` and adapter variants through
//! the interpreter for closure evaluation.

use ori_patterns::{EvalError, IteratorValue};

use crate::{ControlAction, EvalResult, Value};

use super::super::Interpreter;

impl Interpreter<'_> {
    /// Advance an iterator by one step, handling both source and adapter variants.
    ///
    /// Returns `(Option<Value>, IteratorValue)` — the yielded item and the
    /// advanced iterator state.
    pub(in crate::interpreter) fn eval_iter_next(
        &mut self,
        iter_val: IteratorValue,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        match iter_val {
            // Source variants — pure, no interpreter needed
            IteratorValue::List { .. }
            | IteratorValue::Range { .. }
            | IteratorValue::Map { .. }
            | IteratorValue::Set { .. }
            | IteratorValue::Str { .. }
            | IteratorValue::Repeat { .. } => {
                let (item, new_iter) = iter_val.next();
                Ok((item, new_iter))
            }

            adapter @ (IteratorValue::Mapped { .. } | IteratorValue::Filtered { .. }) => {
                self.eval_iter_next_transform(adapter)
            }

            adapter @ (IteratorValue::TakeN { .. }
            | IteratorValue::SkipN { .. }
            | IteratorValue::Enumerated { .. }) => self.eval_iter_next_stateful(adapter),

            adapter @ (IteratorValue::Zipped { .. } | IteratorValue::Chained { .. }) => {
                self.eval_iter_next_composed(adapter)
            }

            // Flattened: advance inner; if exhausted, advance source for new inner
            IteratorValue::Flattened { source, inner } => {
                self.eval_iter_next_flattened(*source, inner)
            }

            // Cycled: first pass buffers items; subsequent passes replay from buffer
            IteratorValue::Cycled {
                source,
                mut buffer,
                buf_pos,
            } => self.eval_iter_next_cycled(source, &mut buffer, buf_pos),

            // Reversed: delegate to next_back on source
            IteratorValue::Reversed { source } => {
                let (item, new_source) = self.eval_iter_next_back(*source)?;
                Ok((
                    item,
                    IteratorValue::Reversed {
                        source: Box::new(new_source),
                    },
                ))
            }
        }
    }

    fn eval_iter_next_transform(
        &mut self,
        iter_val: IteratorValue,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        match iter_val {
            IteratorValue::Mapped { source, transform } => {
                let (item, new_source) = self.eval_iter_next(*source)?;
                let mapped = match item {
                    Some(value) => Some(self.eval_call(&transform, &[value])?),
                    None => None,
                };
                Ok((
                    mapped,
                    IteratorValue::Mapped {
                        source: Box::new(new_source),
                        transform,
                    },
                ))
            }
            IteratorValue::Filtered { source, predicate } => {
                let mut current = *source;
                loop {
                    let (item, new_source) = self.eval_iter_next(current)?;
                    match item {
                        Some(value) => {
                            let keep = self.eval_call(&predicate, std::slice::from_ref(&value))?;
                            if keep.is_truthy() {
                                return Ok((
                                    Some(value),
                                    IteratorValue::Filtered {
                                        source: Box::new(new_source),
                                        predicate,
                                    },
                                ));
                            }
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
            _ => unreachable!("non-transform adapter in transform dispatch"),
        }
    }

    fn eval_iter_next_stateful(
        &mut self,
        iter_val: IteratorValue,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        match iter_val {
            IteratorValue::TakeN { source, remaining } => {
                if remaining == 0 {
                    return Ok((None, IteratorValue::TakeN { source, remaining }));
                }
                let (item, new_source) = self.eval_iter_next(*source)?;
                Ok((
                    item,
                    IteratorValue::TakeN {
                        source: Box::new(new_source),
                        remaining: remaining.saturating_sub(1),
                    },
                ))
            }
            IteratorValue::SkipN { source, remaining } => {
                let mut current = *source;
                for _ in 0..remaining {
                    let (item, new_source) = self.eval_iter_next(current)?;
                    current = new_source;
                    if item.is_none() {
                        return Ok((
                            None,
                            IteratorValue::SkipN {
                                source: Box::new(current),
                                remaining: 0,
                            },
                        ));
                    }
                }
                let (item, new_source) = self.eval_iter_next(current)?;
                Ok((
                    item,
                    IteratorValue::SkipN {
                        source: Box::new(new_source),
                        remaining: 0,
                    },
                ))
            }
            IteratorValue::Enumerated { source, index } => {
                let (item, new_source) = self.eval_iter_next(*source)?;
                let next_index = if item.is_some() {
                    index.saturating_add(1)
                } else {
                    index
                };
                let index_value = i64::try_from(index)
                    .map_err(|_| EvalError::new("iterator index exceeds the int range"))?;
                let item = item.map(|value| Value::tuple(vec![Value::int(index_value), value]));
                Ok((
                    item,
                    IteratorValue::Enumerated {
                        source: Box::new(new_source),
                        index: next_index,
                    },
                ))
            }
            _ => unreachable!("non-stateful adapter in stateful dispatch"),
        }
    }

    fn eval_iter_next_composed(
        &mut self,
        iter_val: IteratorValue,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        match iter_val {
            IteratorValue::Zipped { left, right } => {
                let (left_item, new_left) = self.eval_iter_next(*left)?;
                let Some(left_value) = left_item else {
                    return Ok((
                        None,
                        IteratorValue::Zipped {
                            left: Box::new(new_left),
                            right,
                        },
                    ));
                };
                let (right_item, new_right) = self.eval_iter_next(*right)?;
                let item =
                    right_item.map(|right_value| Value::tuple(vec![left_value, right_value]));
                Ok((
                    item,
                    IteratorValue::Zipped {
                        left: Box::new(new_left),
                        right: Box::new(new_right),
                    },
                ))
            }
            IteratorValue::Chained {
                first,
                second,
                first_done,
            } => {
                if first_done {
                    let (item, new_second) = self.eval_iter_next(*second)?;
                    return Ok((
                        item,
                        IteratorValue::Chained {
                            first,
                            second: Box::new(new_second),
                            first_done,
                        },
                    ));
                }
                let (item, new_first) = self.eval_iter_next(*first)?;
                if item.is_some() {
                    return Ok((
                        item,
                        IteratorValue::Chained {
                            first: Box::new(new_first),
                            second,
                            first_done,
                        },
                    ));
                }
                let (item, new_second) = self.eval_iter_next(*second)?;
                Ok((
                    item,
                    IteratorValue::Chained {
                        first: Box::new(new_first),
                        second: Box::new(new_second),
                        first_done: true,
                    },
                ))
            }
            _ => unreachable!("non-composed adapter in composed dispatch"),
        }
    }

    /// Pack an advance result `(item?, new_iter)` into the Ori iterator-protocol
    /// tuple `(T?, Iterator<T>)` — the single return shape shared by `next()` and
    /// `next_back()`.
    fn pack_iter_advance_tuple((maybe_item, new_iter): (Option<Value>, IteratorValue)) -> Value {
        let option_val = match maybe_item {
            Some(v) => Value::some(v),
            None => Value::None,
        };
        Value::tuple(vec![option_val, Value::iterator(new_iter)])
    }

    /// `next()` returns `(T?, Iterator<T>)` tuple for the Ori protocol.
    pub(in crate::interpreter) fn eval_iter_next_as_tuple(
        &mut self,
        iter_val: IteratorValue,
    ) -> EvalResult {
        Ok(Self::pack_iter_advance_tuple(
            self.eval_iter_next(iter_val)?,
        ))
    }

    /// Advance an iterator from the back by one step.
    ///
    /// Only valid for double-ended variants (List, Range, Str) and adapters
    /// whose source is double-ended (Mapped, Filtered).
    pub(in crate::interpreter) fn eval_iter_next_back(
        &mut self,
        iter_val: IteratorValue,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        match iter_val {
            // Source variants — pure, delegate to IteratorValue::next_back()
            IteratorValue::List { .. }
            | IteratorValue::Range { .. }
            | IteratorValue::Str { .. } => {
                let (item, new_iter) = iter_val.next_back();
                Ok((item, new_iter))
            }

            // Mapped: get next_back from source, apply transform
            IteratorValue::Mapped { source, transform } => {
                let (item, new_source) = self.eval_iter_next_back(*source)?;
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

            // Filtered: loop source.next_back() until predicate passes
            IteratorValue::Filtered { source, predicate } => {
                let mut current = *source;
                loop {
                    let (item, new_source) = self.eval_iter_next_back(current)?;
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

            // Reversed: next_back on reversed delegates to next on source
            IteratorValue::Reversed { source } => {
                let (item, new_source) = self.eval_iter_next(*source)?;
                Ok((
                    item,
                    IteratorValue::Reversed {
                        source: Box::new(new_source),
                    },
                ))
            }

            // Non-double-ended variants — runtime error
            _ => {
                use crate::errors::wrong_arg_type;
                Err(wrong_arg_type("next_back", "double-ended iterator").into())
            }
        }
    }

    /// `next_back()` returns `(T?, Iterator<T>)` tuple for the Ori protocol.
    pub(in crate::interpreter) fn eval_iter_next_back_as_tuple(
        &mut self,
        iter_val: IteratorValue,
    ) -> EvalResult {
        Ok(Self::pack_iter_advance_tuple(
            self.eval_iter_next_back(iter_val)?,
        ))
    }
}
