//! Stateful iterator advancement — `Flattened` and `Cycled` variants.
//!
//! These two `IteratorValue` variants maintain complex internal state across
//! `next()` calls and are extracted here to keep `next.rs` focused on the
//! main dispatch table.

use ori_patterns::{ControlAction, IteratorValue, Value};

use super::super::super::Interpreter;

impl Interpreter<'_> {
    /// Advance a `Flattened` iterator.
    ///
    /// Loops: try inner → if exhausted, advance source → convert to iterator → set as inner.
    pub(super) fn eval_iter_next_flattened(
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
    pub(super) fn eval_iter_next_cycled(
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
}
