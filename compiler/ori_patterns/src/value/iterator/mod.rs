//! Iterator value type for the Ori interpreter.
//!
//! Implements functional iterator semantics: each `next()` call returns
//! `(Option<Item>, Iterator)` — a new iterator value with advanced position.
//! Collection data is shared via `Heap<T>` (Arc), so cloning an iterator
//! only copies the position field.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

use super::heap::Heap;
use super::Value;

/// Iterator value wrapping per-collection state.
///
/// Each variant carries the source collection behind a `Heap<T>` (shared,
/// O(1) clone) plus a position index that advances on each `next()`.
#[derive(Clone)]
pub enum IteratorValue {
    /// Iterator over a list's elements.
    List { items: Heap<Vec<Value>>, pos: usize },
    /// Iterator over an integer range.
    Range {
        current: i64,
        end: i64,
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
    /// Iterator over a string's characters.
    Str {
        data: Heap<Cow<'static, str>>,
        byte_pos: usize,
    },
    /// Lazy map adapter: applies `transform` to each item yielded by `source`.
    ///
    /// `transform` is `Box<Value>` to break the `Value ↔ IteratorValue` drop-check cycle.
    Mapped {
        source: Box<IteratorValue>,
        transform: Box<Value>,
    },
    /// Lazy filter adapter: yields only items from `source` matching `predicate`.
    ///
    /// `predicate` is `Box<Value>` to break the `Value ↔ IteratorValue` drop-check cycle.
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
}

impl IteratorValue {
    /// Advance the iterator, returning `(Option<Item>, new_iterator)`.
    ///
    /// This is the core functional iteration primitive. The returned iterator
    /// has the position advanced past the yielded element.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "pos/byte_pos increments are guarded by bounds checks; range step is user-provided i64"
    )]
    pub fn next(&self) -> (Option<Value>, IteratorValue) {
        match self {
            IteratorValue::List { items, pos } => {
                if *pos < items.len() {
                    let val = items[*pos].clone();
                    let new_iter = IteratorValue::List {
                        items: items.clone(),
                        pos: pos + 1,
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
                let in_bounds = if *inclusive {
                    if *step > 0 {
                        *current <= *end
                    } else {
                        *current >= *end
                    }
                } else if *step > 0 {
                    *current < *end
                } else {
                    *current > *end
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

            IteratorValue::Str { data, byte_pos } => {
                let remaining = &data[*byte_pos..];
                if let Some(ch) = remaining.chars().next() {
                    let new_iter = IteratorValue::Str {
                        data: data.clone(),
                        byte_pos: byte_pos + ch.len_utf8(),
                    };
                    (Some(Value::Char(ch)), new_iter)
                } else {
                    (None, self.clone())
                }
            }

            // Adapter variants require interpreter access to call closures.
            // They must be advanced via `Interpreter::eval_iter_next()`, not
            // this pure `next()` method.
            IteratorValue::Mapped { .. }
            | IteratorValue::Filtered { .. }
            | IteratorValue::TakeN { .. }
            | IteratorValue::SkipN { .. } => {
                unreachable!(
                    "adapter iterators must be advanced via Interpreter::eval_iter_next(), \
                     not IteratorValue::next()"
                )
            }
        }
    }

    /// Create a list iterator starting at position 0.
    pub fn from_list(items: Heap<Vec<Value>>) -> Self {
        IteratorValue::List { items, pos: 0 }
    }

    /// Create a range iterator.
    pub fn from_range(start: i64, end: i64, step: i64, inclusive: bool) -> Self {
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

    /// Create a string character iterator.
    pub fn from_string(data: Heap<Cow<'static, str>>) -> Self {
        IteratorValue::Str { data, byte_pos: 0 }
    }
}

impl fmt::Debug for IteratorValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IteratorValue::List { pos, items } => {
                write!(f, "ListIterator(pos={}, len={})", pos, items.len())
            }
            IteratorValue::Range {
                current,
                end,
                inclusive,
                ..
            } => {
                let op = if *inclusive { "..=" } else { ".." };
                write!(f, "RangeIterator({current}{op}{end})")
            }
            IteratorValue::Map { pos, entries } => {
                write!(f, "MapIterator(pos={}, len={})", pos, entries.len())
            }
            IteratorValue::Set { pos, items } => {
                write!(f, "SetIterator(pos={}, len={})", pos, items.len())
            }
            IteratorValue::Str { byte_pos, data } => {
                write!(f, "StrIterator(byte_pos={}, len={})", byte_pos, data.len())
            }
            IteratorValue::Mapped { source, .. } => {
                write!(f, "MappedIterator({source:?})")
            }
            IteratorValue::Filtered { source, .. } => {
                write!(f, "FilteredIterator({source:?})")
            }
            IteratorValue::TakeN {
                source, remaining, ..
            } => {
                write!(f, "TakeIterator(remaining={remaining}, {source:?})")
            }
            IteratorValue::SkipN {
                source, remaining, ..
            } => {
                write!(f, "SkipIterator(remaining={remaining}, {source:?})")
            }
        }
    }
}

impl PartialEq for IteratorValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                IteratorValue::List { items: a, pos: pa },
                IteratorValue::List { items: b, pos: pb },
            )
            | (
                IteratorValue::Set { items: a, pos: pa },
                IteratorValue::Set { items: b, pos: pb },
            ) => pa == pb && a == b,
            (
                IteratorValue::Range {
                    current: ca,
                    end: ea,
                    step: sa,
                    inclusive: ia,
                },
                IteratorValue::Range {
                    current: cb,
                    end: eb,
                    step: sb,
                    inclusive: ib,
                },
            ) => ca == cb && ea == eb && sa == sb && ia == ib,
            (
                IteratorValue::Map {
                    entries: a,
                    pos: pa,
                },
                IteratorValue::Map {
                    entries: b,
                    pos: pb,
                },
            ) => pa == pb && a == b,
            (
                IteratorValue::Str {
                    data: a,
                    byte_pos: pa,
                },
                IteratorValue::Str {
                    data: b,
                    byte_pos: pb,
                },
            ) => pa == pb && a == b,
            (
                IteratorValue::Mapped {
                    source: sa,
                    transform: ta,
                },
                IteratorValue::Mapped {
                    source: sb,
                    transform: tb,
                },
            ) => sa == sb && ta == tb,
            (
                IteratorValue::Filtered {
                    source: sa,
                    predicate: pa,
                },
                IteratorValue::Filtered {
                    source: sb,
                    predicate: pb,
                },
            ) => sa == sb && pa == pb,
            (
                IteratorValue::TakeN {
                    source: sa,
                    remaining: ra,
                },
                IteratorValue::TakeN {
                    source: sb,
                    remaining: rb,
                },
            )
            | (
                IteratorValue::SkipN {
                    source: sa,
                    remaining: ra,
                },
                IteratorValue::SkipN {
                    source: sb,
                    remaining: rb,
                },
            ) => sa == sb && ra == rb,
            _ => false,
        }
    }
}

impl Eq for IteratorValue {}

impl std::hash::Hash for IteratorValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            IteratorValue::List { pos, .. }
            | IteratorValue::Map { pos, .. }
            | IteratorValue::Set { pos, .. } => pos.hash(state),
            IteratorValue::Range {
                current,
                end,
                step,
                inclusive,
            } => {
                current.hash(state);
                end.hash(state);
                step.hash(state);
                inclusive.hash(state);
            }
            IteratorValue::Str { byte_pos, .. } => byte_pos.hash(state),
            IteratorValue::Mapped {
                source, transform, ..
            } => {
                source.hash(state);
                transform.hash(state);
            }
            IteratorValue::Filtered {
                source, predicate, ..
            } => {
                source.hash(state);
                predicate.hash(state);
            }
            IteratorValue::TakeN {
                source, remaining, ..
            }
            | IteratorValue::SkipN {
                source, remaining, ..
            } => {
                source.hash(state);
                remaining.hash(state);
            }
        }
    }
}

#[cfg(test)]
mod tests;
