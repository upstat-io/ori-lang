//! Core iteration methods: `next()` and `next_back()`.

use super::super::Value;
use super::IteratorValue;

impl IteratorValue {
    /// Advance the iterator, returning `(Option<Item>, new_iterator)`.
    ///
    /// This is the core functional iteration primitive. The returned iterator
    /// has the position advanced past the yielded element.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "pos/byte_pos increments are guarded by bounds checks; range step is user-provided i64"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive IteratorValue next-element dispatch"
    )]
    pub fn next(&self) -> (Option<Value>, IteratorValue) {
        match self {
            IteratorValue::List { items, front, back } => {
                if *front < *back {
                    let val = items[*front].clone();
                    let new_iter = IteratorValue::List {
                        items: items.clone(),
                        front: front + 1,
                        back: *back,
                    };
                    (Some(val), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            IteratorValue::Range {
                current,
                end,
                step,
                inclusive,
            } => {
                let in_bounds = match end {
                    // Unbounded: always in bounds (if step != 0)
                    None => *step != 0,
                    Some(end_val) => {
                        if *inclusive {
                            if *step > 0 {
                                *current <= *end_val
                            } else {
                                *current >= *end_val
                            }
                        } else if *step > 0 {
                            *current < *end_val
                        } else {
                            *current > *end_val
                        }
                    }
                };

                if in_bounds {
                    let val = Value::int(*current);
                    let new_iter = IteratorValue::Range {
                        current: current + step,
                        end: *end,
                        step: *step,
                        inclusive: *inclusive,
                    };
                    (Some(val), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            IteratorValue::Map { entries, pos } => {
                if *pos < entries.len() {
                    let (key, val) = &entries[*pos];
                    let tuple = Value::tuple(vec![Value::string(key.clone()), val.clone()]);
                    let new_iter = IteratorValue::Map {
                        entries: entries.clone(),
                        pos: pos + 1,
                    };
                    (Some(tuple), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            IteratorValue::Set { items, pos } => {
                if *pos < items.len() {
                    let val = items[*pos].clone();
                    let new_iter = IteratorValue::Set {
                        items: items.clone(),
                        pos: pos + 1,
                    };
                    (Some(val), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            IteratorValue::Str {
                data,
                front_pos,
                back_pos,
            } => {
                let remaining = &data[*front_pos..*back_pos];
                if let Some(ch) = remaining.chars().next() {
                    let new_iter = IteratorValue::Str {
                        data: data.clone(),
                        front_pos: front_pos + ch.len_utf8(),
                        back_pos: *back_pos,
                    };
                    (Some(Value::Char(ch)), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            // Repeat: always yields a clone of the stored value
            IteratorValue::Repeat { value } => (Some(Value::clone(value)), self.clone()),

            // Adapter variants require interpreter access to call closures.
            // They must be advanced via `Interpreter::eval_iter_next()`, not
            // this pure `next()` method.
            IteratorValue::Mapped { .. }
            | IteratorValue::Filtered { .. }
            | IteratorValue::TakeN { .. }
            | IteratorValue::SkipN { .. }
            | IteratorValue::Enumerated { .. }
            | IteratorValue::Zipped { .. }
            | IteratorValue::Chained { .. }
            | IteratorValue::Flattened { .. }
            | IteratorValue::Cycled { .. }
            | IteratorValue::Reversed { .. } => {
                unreachable!(
                    "adapter iterators must be advanced via Interpreter::eval_iter_next(), \
                     not IteratorValue::next()"
                )
            }
        }
    }

    /// Advance the iterator from the back, returning `(Option<Item>, new_iterator)`.
    ///
    /// Only supported on double-ended variants (List, Range, Str) and adapters
    /// whose source is double-ended (Mapped, Filtered). For other variants,
    /// use `Interpreter::eval_iter_next_back()` which handles closure-based adapters.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "back/front_pos decrements are guarded by bounds checks; range arithmetic on aligned values"
    )]
    pub fn next_back(&self) -> (Option<Value>, IteratorValue) {
        match self {
            IteratorValue::List { items, front, back } => {
                if *front < *back {
                    let val = items[back - 1].clone();
                    let new_iter = IteratorValue::List {
                        items: items.clone(),
                        front: *front,
                        back: back - 1,
                    };
                    (Some(val), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            IteratorValue::Range {
                current,
                end,
                step,
                inclusive,
            } => {
                // Safety: unbounded range iterators are not double-ended —
                // caller must check is_double_ended() first.
                let Some(&end_val) = end.as_ref() else {
                    debug_assert!(false, "next_back() called on unbounded range");
                    return (None, self.clone());
                };
                let n = super::range_len(*current, end_val, *step, *inclusive);
                if n == 0 {
                    return (None, self.clone());
                }
                // Compute last aligned value in the sequence
                #[expect(
                    clippy::cast_possible_wrap,
                    reason = "n-1 fits in i64 since range_len is derived from i64 arithmetic"
                )]
                let last = current + (n as i64 - 1) * step;
                let new_iter = IteratorValue::Range {
                    current: *current,
                    end: Some(last),
                    step: *step,
                    // After removing the last element, use exclusive bound at `last`
                    inclusive: false,
                };
                (Some(Value::int(last)), new_iter)
            }

            IteratorValue::Str {
                data,
                front_pos,
                back_pos,
            } => {
                let remaining = &data[*front_pos..*back_pos];
                if let Some(ch) = remaining.chars().next_back() {
                    let new_iter = IteratorValue::Str {
                        data: data.clone(),
                        front_pos: *front_pos,
                        back_pos: back_pos - ch.len_utf8(),
                    };
                    (Some(Value::Char(ch)), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            // Map, Set, and Repeat are not double-ended
            IteratorValue::Map { .. }
            | IteratorValue::Set { .. }
            | IteratorValue::Repeat { .. } => {
                unreachable!(
                    "Map/Set/Repeat iterators are not double-ended — \
                     caller must check is_double_ended() first"
                )
            }

            // Adapter variants require interpreter access to call closures.
            IteratorValue::Mapped { .. }
            | IteratorValue::Filtered { .. }
            | IteratorValue::TakeN { .. }
            | IteratorValue::SkipN { .. }
            | IteratorValue::Enumerated { .. }
            | IteratorValue::Zipped { .. }
            | IteratorValue::Chained { .. }
            | IteratorValue::Flattened { .. }
            | IteratorValue::Cycled { .. }
            | IteratorValue::Reversed { .. } => {
                unreachable!(
                    "adapter iterators must be advanced via Interpreter::eval_iter_next_back(), \
                     not IteratorValue::next_back()"
                )
            }
        }
    }
}
