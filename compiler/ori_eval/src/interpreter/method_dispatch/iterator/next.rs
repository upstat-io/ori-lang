//! Core iterator advancement logic.
//!
//! `eval_iter_next()` handles all `IteratorValue` variants, routing pure
//! source variants to `IteratorValue::next()` and adapter variants through
//! the interpreter for closure evaluation.

use ori_patterns::IteratorValue;

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

            // Enumerated: pair each item with its 0-based index
            IteratorValue::Enumerated { source, index } => {
                let (item, new_source) = self.eval_iter_next(*source)?;
                match item {
                    Some(val) => {
                        #[expect(
                            clippy::cast_possible_wrap,
                            reason = "enumerate index won't exceed i64::MAX in practice"
                        )]
                        let idx_val = Value::int(index as i64);
                        let pair = Value::tuple(vec![idx_val, val]);
                        Ok((
                            Some(pair),
                            IteratorValue::Enumerated {
                                source: Box::new(new_source),
                                index: index.saturating_add(1),
                            },
                        ))
                    }
                    None => Ok((
                        None,
                        IteratorValue::Enumerated {
                            source: Box::new(new_source),
                            index,
                        },
                    )),
                }
            }

            // Zipped: advance both, yield tuple or stop if either exhausted
            IteratorValue::Zipped { left, right } => {
                let (left_item, new_left) = self.eval_iter_next(*left)?;
                match left_item {
                    Some(l) => {
                        let (right_item, new_right) = self.eval_iter_next(*right)?;
                        match right_item {
                            Some(r) => Ok((
                                Some(Value::tuple(vec![l, r])),
                                IteratorValue::Zipped {
                                    left: Box::new(new_left),
                                    right: Box::new(new_right),
                                },
                            )),
                            None => Ok((
                                None,
                                IteratorValue::Zipped {
                                    left: Box::new(new_left),
                                    right: Box::new(new_right),
                                },
                            )),
                        }
                    }
                    None => Ok((
                        None,
                        IteratorValue::Zipped {
                            left: Box::new(new_left),
                            right,
                        },
                    )),
                }
            }

            // Chained: exhaust first, then yield from second
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
                            first_done: true,
                        },
                    ));
                }
                let (item, new_first) = self.eval_iter_next(*first)?;
                if let Some(val) = item {
                    Ok((
                        Some(val),
                        IteratorValue::Chained {
                            first: Box::new(new_first),
                            second,
                            first_done: false,
                        },
                    ))
                } else {
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
        }
    }

    /// Advance a `Flattened` iterator.
    ///
    /// Loops: try inner → if exhausted, advance source → convert to iterator → set as inner.
    fn eval_iter_next_flattened(
        &mut self,
        source: IteratorValue,
        inner: Option<Box<IteratorValue>>,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        let mut current_source = source;
        let mut current_inner = inner;

        loop {
            // Try advancing inner iterator
            if let Some(inner_iter) = current_inner {
                let (item, new_inner) = self.eval_iter_next(*inner_iter)?;
                if item.is_some() {
                    return Ok((
                        item,
                        IteratorValue::Flattened {
                            source: Box::new(current_source),
                            inner: Some(Box::new(new_inner)),
                        },
                    ));
                }
                // Inner exhausted — fall through to advance source
            }

            // Advance source for a new inner iterator
            let (source_item, new_source) = self.eval_iter_next(current_source)?;
            match source_item {
                Some(val) => {
                    match IteratorValue::from_value(&val) {
                        Some(new_inner_iter) => {
                            current_source = new_source;
                            current_inner = Some(Box::new(new_inner_iter));
                            // Loop to try advancing the new inner
                        }
                        None => {
                            // Non-iterable item — yield it directly (like Rust's flatten for Option)
                            return Ok((
                                Some(val),
                                IteratorValue::Flattened {
                                    source: Box::new(new_source),
                                    inner: None,
                                },
                            ));
                        }
                    }
                }
                None => {
                    return Ok((
                        None,
                        IteratorValue::Flattened {
                            source: Box::new(new_source),
                            inner: None,
                        },
                    ));
                }
            }
        }
    }

    /// Advance a `Cycled` iterator.
    ///
    /// First pass: consume source, buffer items. When source exhausted, replay
    /// from buffer. If source is empty, cycle yields nothing forever.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "buf_pos modulo buffer.len() is always in bounds; buffer.len() > 0 guarded"
    )]
    fn eval_iter_next_cycled(
        &mut self,
        source: Option<Box<IteratorValue>>,
        buffer: &mut Vec<Value>,
        buf_pos: usize,
    ) -> Result<(Option<Value>, IteratorValue), ControlAction> {
        if let Some(src) = source {
            // First pass — consuming from source
            let (item, new_source) = self.eval_iter_next(*src)?;
            match item {
                Some(val) => {
                    buffer.push(val.clone());
                    Ok((
                        Some(val),
                        IteratorValue::Cycled {
                            source: Some(Box::new(new_source)),
                            buffer: std::mem::take(buffer),
                            buf_pos: 0,
                        },
                    ))
                }
                None => {
                    // Source exhausted — switch to replay mode
                    if buffer.is_empty() {
                        // Empty source → cycle is permanently exhausted
                        Ok((
                            None,
                            IteratorValue::Cycled {
                                source: None,
                                buffer: Vec::new(),
                                buf_pos: 0,
                            },
                        ))
                    } else {
                        // Start replaying from position 0
                        let val = buffer[0].clone();
                        Ok((
                            Some(val),
                            IteratorValue::Cycled {
                                source: None,
                                buffer: std::mem::take(buffer),
                                buf_pos: 1,
                            },
                        ))
                    }
                }
            }
        } else {
            // Replay mode — cycle through buffer
            if buffer.is_empty() {
                return Ok((
                    None,
                    IteratorValue::Cycled {
                        source: None,
                        buffer: std::mem::take(buffer),
                        buf_pos: 0,
                    },
                ));
            }
            let actual_pos = buf_pos % buffer.len();
            let val = buffer[actual_pos].clone();
            Ok((
                Some(val),
                IteratorValue::Cycled {
                    source: None,
                    buffer: std::mem::take(buffer),
                    buf_pos: actual_pos + 1,
                },
            ))
        }
    }

    /// `next()` returns `(T?, Iterator<T>)` tuple for the Ori protocol.
    pub(in crate::interpreter) fn eval_iter_next_as_tuple(
        &mut self,
        iter_val: IteratorValue,
    ) -> EvalResult {
        let (maybe_item, new_iter) = self.eval_iter_next(iter_val)?;
        let option_val = match maybe_item {
            Some(v) => Value::some(v),
            None => Value::None,
        };
        Ok(Value::tuple(vec![option_val, Value::iterator(new_iter)]))
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
        let (maybe_item, new_iter) = self.eval_iter_next_back(iter_val)?;
        let option_val = match maybe_item {
            Some(v) => Value::some(v),
            None => Value::None,
        };
        Ok(Value::tuple(vec![option_val, Value::iterator(new_iter)]))
    }
}
