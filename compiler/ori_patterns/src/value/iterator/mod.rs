//! Iterator value type for the Ori interpreter.
//!
//! Implements functional iterator semantics: each `next()` call returns
//! `(Option<Item>, Iterator)` — a new iterator value with advanced position.
//! Collection data is shared via `Heap<T>` (Arc), so cloning an iterator
//! only copies the position field.

mod next;
mod size_hint;
mod traits;

use std::borrow::Cow;
use std::collections::BTreeMap;

use super::heap::Heap;
use super::Value;

/// Compute the number of remaining elements in a range.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "division by non-zero step; subtraction of same-sign values"
)]
fn range_len(current: i64, end: i64, step: i64, inclusive: bool) -> usize {
    if step == 0 {
        return 0;
    }
    let diff = if step > 0 {
        end.saturating_sub(current)
    } else {
        current.saturating_sub(end)
    };
    if diff < 0 {
        return 0;
    }
    #[expect(
        clippy::cast_sign_loss,
        reason = "diff is non-negative (guarded above)"
    )]
    let abs_diff = diff as u64;
    let abs_step = step.unsigned_abs();

    let count = abs_diff / abs_step;
    let has_remainder = inclusive || !abs_diff.is_multiple_of(abs_step);
    let total = if has_remainder { count + 1 } else { count };
    usize::try_from(total).unwrap_or(usize::MAX)
}

/// Iterator value wrapping per-collection state.
///
/// Each variant carries the source collection behind a `Heap<T>` (shared,
/// O(1) clone) plus a position index that advances on each `next()`.
#[derive(Clone)]
pub enum IteratorValue {
    /// Iterator over a list's elements (double-ended: front advances forward, back backward).
    List {
        items: Heap<Vec<Value>>,
        front: usize,
        back: usize,
    },
    /// Iterator over an integer range.
    ///
    /// When `end` is `None`, the range is unbounded (infinite in the step direction).
    Range {
        current: i64,
        end: Option<i64>,
        step: i64,
        inclusive: bool,
    },
    /// Iterator over pre-collected map entries `(key, value)`.
    ///
    /// `BTreeMap` iterators are stateful/mutable, incompatible with functional
    /// `next()`. At `.iter()` time, entries are collected into a
    /// `Vec<(String, Value)>` (O(n) once), then iterated by position.
    Map {
        entries: Heap<Vec<(String, Value)>>,
        pos: usize,
    },
    /// Iterator over a set's elements (collected to Vec for positional access).
    Set { items: Heap<Vec<Value>>, pos: usize },
    /// Iterator over a string's characters (double-ended: front/back byte positions).
    Str {
        data: Heap<Cow<'static, str>>,
        front_pos: usize,
        back_pos: usize,
    },
    /// Lazy map adapter: applies `transform` to each item yielded by `source`.
    ///
    /// `transform` is `Box<Value>` to break the `Value <-> IteratorValue` drop-check cycle.
    Mapped {
        source: Box<IteratorValue>,
        transform: Box<Value>,
    },
    /// Lazy filter adapter: yields only items from `source` matching `predicate`.
    ///
    /// `predicate` is `Box<Value>` to break the `Value <-> IteratorValue` drop-check cycle.
    Filtered {
        source: Box<IteratorValue>,
        predicate: Box<Value>,
    },
    /// Take adapter: yields at most `remaining` items from `source`.
    TakeN {
        source: Box<IteratorValue>,
        remaining: usize,
    },
    /// Skip adapter: skips first `remaining` items, then yields from `source`.
    SkipN {
        source: Box<IteratorValue>,
        remaining: usize,
    },
    /// Enumerate adapter: pairs each item with its 0-based index.
    Enumerated {
        source: Box<IteratorValue>,
        index: usize,
    },
    /// Zip adapter: yields `(left_item, right_item)` tuples until either exhausts.
    Zipped {
        left: Box<IteratorValue>,
        right: Box<IteratorValue>,
    },
    /// Chain adapter: yields all items from `first`, then all from `second`.
    Chained {
        first: Box<IteratorValue>,
        second: Box<IteratorValue>,
        first_done: bool,
    },
    /// Flatten adapter: yields items from nested iterators.
    ///
    /// `source` yields iterable values; `inner` is the current sub-iterator.
    Flattened {
        source: Box<IteratorValue>,
        inner: Option<Box<IteratorValue>>,
    },
    /// Cycle adapter: replays items infinitely by buffering the first pass.
    ///
    /// While `source` is `Some`, items are consumed and buffered. Once exhausted,
    /// subsequent iterations replay from `buffer` starting at `buf_pos`.
    Cycled {
        source: Option<Box<IteratorValue>>,
        buffer: Vec<Value>,
        buf_pos: usize,
    },
    /// Reverse adapter: swaps `next()` and `next_back()` on a double-ended source.
    ///
    /// `source` must be double-ended. Calling `next()` on a `Reversed` iterator
    /// delegates to `source.next_back()`, and vice versa.
    Reversed { source: Box<IteratorValue> },
    /// Repeat: infinite iterator that yields the same value on every `next()`.
    ///
    /// Created by the `repeat(value)` prelude function. Each call clones the
    /// stored value. Not double-ended (infinite in one direction only).
    Repeat { value: Box<Value> },
}

impl IteratorValue {
    /// Returns `true` if this iterator supports `next_back()`.
    ///
    /// Double-ended: `List`, `Range`, `Str`, and `Mapped`/`Filtered` adapters
    /// wrapping a double-ended source.
    pub fn is_double_ended(&self) -> bool {
        match self {
            // Source variants and Reversed are always double-ended
            IteratorValue::List { .. }
            | IteratorValue::Str { .. }
            | IteratorValue::Reversed { .. } => true,

            // Bounded ranges are double-ended; unbounded are not
            IteratorValue::Range { end, .. } => end.is_some(),

            // Mapped/Filtered propagate from source
            IteratorValue::Mapped { source, .. } | IteratorValue::Filtered { source, .. } => {
                source.is_double_ended()
            }

            // Map/Set (unordered), Repeat (infinite), and other adapters are not double-ended
            IteratorValue::Map { .. }
            | IteratorValue::Set { .. }
            | IteratorValue::TakeN { .. }
            | IteratorValue::SkipN { .. }
            | IteratorValue::Enumerated { .. }
            | IteratorValue::Zipped { .. }
            | IteratorValue::Chained { .. }
            | IteratorValue::Flattened { .. }
            | IteratorValue::Cycled { .. }
            | IteratorValue::Repeat { .. } => false,
        }
    }

    /// Create a list iterator spanning all elements.
    pub fn from_list(items: Heap<Vec<Value>>) -> Self {
        let back = items.len();
        IteratorValue::List {
            items,
            front: 0,
            back,
        }
    }

    /// Create a range iterator.
    pub fn from_range(start: i64, end: Option<i64>, step: i64, inclusive: bool) -> Self {
        IteratorValue::Range {
            current: start,
            end,
            step,
            inclusive,
        }
    }

    /// Create a map iterator from pre-collected entries.
    pub fn from_map(map: &BTreeMap<String, Value>) -> Self {
        let entries: Vec<(String, Value)> =
            map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        IteratorValue::Map {
            entries: Heap::new(entries),
            pos: 0,
        }
    }

    /// Create a set iterator (re-uses list storage since sets are Vec-backed).
    pub fn from_set(items: Heap<Vec<Value>>) -> Self {
        IteratorValue::Set { items, pos: 0 }
    }

    /// Create a string character iterator spanning the entire string.
    pub fn from_string(data: Heap<Cow<'static, str>>) -> Self {
        let back_pos = data.len();
        IteratorValue::Str {
            data,
            front_pos: 0,
            back_pos,
        }
    }

    /// Create an infinite repeat iterator that yields the same value forever.
    pub fn from_repeat(value: Value) -> Self {
        IteratorValue::Repeat {
            value: Box::new(value),
        }
    }

    /// Create a list iterator from a `ListData`, respecting its offset/len window.
    ///
    /// The iterator shares the same Arc-backed buffer, with `front` and `back`
    /// set to the `ListData`'s visible range.
    pub fn from_list_data(list: &super::list_data::ListData) -> Self {
        IteratorValue::List {
            items: list.backing().clone(),
            front: list.offset(),
            back: list.offset().saturating_add(list.len()),
        }
    }

    /// Convert an iterable `Value` to an `IteratorValue`, if possible.
    ///
    /// Used by `flatten` to turn each yielded value into a sub-iterator.
    /// Returns `None` for non-iterable values (int, bool, etc.).
    pub fn from_value(val: &Value) -> Option<Self> {
        match val {
            Value::List(list) => Some(Self::from_list_data(list)),
            Value::Map(map) => Some(Self::from_map(map)),
            Value::Str(s) => Some(Self::from_string(s.clone())),
            Value::Range(r) => Some(Self::from_range(r.start, r.end, r.step, r.inclusive)),
            Value::Set(items) => {
                let values: Vec<Value> = items.values().cloned().collect();
                Some(Self::from_set(Heap::new(values)))
            }
            Value::Iterator(it) => Some(it.clone()),
            // Option<T>: Some(x) -> 1-element list iterator, None -> empty
            Value::Some(v) => Some(Self::from_list(Heap::new(vec![(**v).clone()]))),
            Value::None => Some(Self::from_list(Heap::new(Vec::new()))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
