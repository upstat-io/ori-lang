//! Standard trait implementations for `IteratorValue`.
//!
//! Contains `Debug`, `PartialEq`, `Eq`, and `Hash` implementations.

use std::fmt;

use super::IteratorValue;

impl fmt::Debug for IteratorValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IteratorValue::List { front, back, items } => {
                write!(
                    f,
                    "ListIterator(front={front}, back={back}, len={})",
                    items.len()
                )
            }
            IteratorValue::Range {
                current,
                end,
                inclusive,
                ..
            } => {
                let op = if *inclusive { "..=" } else { ".." };
                match end {
                    Some(end_val) => write!(f, "RangeIterator({current}{op}{end_val})"),
                    None => write!(f, "RangeIterator({current}{op})"),
                }
            }
            IteratorValue::Map { pos, entries } => {
                write!(f, "MapIterator(pos={}, len={})", pos, entries.len())
            }
            IteratorValue::Set { pos, items } => {
                write!(f, "SetIterator(pos={}, len={})", pos, items.len())
            }
            IteratorValue::Str {
                front_pos,
                back_pos,
                data,
            } => {
                write!(
                    f,
                    "StrIterator(front={front_pos}, back={back_pos}, len={})",
                    data.len()
                )
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
            IteratorValue::Enumerated { source, index } => {
                write!(f, "EnumeratedIterator(index={index}, {source:?})")
            }
            IteratorValue::Zipped { left, right } => {
                write!(f, "ZippedIterator({left:?}, {right:?})")
            }
            IteratorValue::Chained {
                first,
                second,
                first_done,
            } => {
                write!(
                    f,
                    "ChainedIterator(first_done={first_done}, {first:?}, {second:?})"
                )
            }
            IteratorValue::Flattened { source, inner } => {
                write!(
                    f,
                    "FlattenedIterator(inner={}, {source:?})",
                    inner.is_some()
                )
            }
            IteratorValue::Cycled {
                source,
                buffer,
                buf_pos,
            } => {
                write!(
                    f,
                    "CycledIterator(buffered={}, buf_pos={buf_pos}, source={})",
                    buffer.len(),
                    source.is_some()
                )
            }
            IteratorValue::Reversed { source } => {
                write!(f, "ReversedIterator({source:?})")
            }
            IteratorValue::Repeat { value } => {
                write!(f, "RepeatIterator({value:?})")
            }
        }
    }
}

impl PartialEq for IteratorValue {
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive IteratorValue equality dispatch"
    )]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                IteratorValue::List {
                    items: a,
                    front: fa,
                    back: ba,
                },
                IteratorValue::List {
                    items: b,
                    front: fb,
                    back: bb,
                },
            ) => fa == fb && ba == bb && a == b,
            (
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
                    front_pos: fa,
                    back_pos: ba,
                },
                IteratorValue::Str {
                    data: b,
                    front_pos: fb,
                    back_pos: bb,
                },
            ) => fa == fb && ba == bb && a == b,
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
            )
            | (
                IteratorValue::Enumerated {
                    source: sa,
                    index: ra,
                },
                IteratorValue::Enumerated {
                    source: sb,
                    index: rb,
                },
            ) => sa == sb && ra == rb,
            (
                IteratorValue::Zipped {
                    left: la,
                    right: ra,
                },
                IteratorValue::Zipped {
                    left: lb,
                    right: rb,
                },
            ) => la == lb && ra == rb,
            (
                IteratorValue::Chained {
                    first: fa,
                    second: sa,
                    first_done: da,
                },
                IteratorValue::Chained {
                    first: fb,
                    second: sb,
                    first_done: db,
                },
            ) => da == db && fa == fb && sa == sb,
            (
                IteratorValue::Flattened {
                    source: sa,
                    inner: ia,
                },
                IteratorValue::Flattened {
                    source: sb,
                    inner: ib,
                },
            ) => sa == sb && ia == ib,
            (
                IteratorValue::Cycled {
                    source: sa,
                    buffer: ba,
                    buf_pos: pa,
                },
                IteratorValue::Cycled {
                    source: sb,
                    buffer: bb,
                    buf_pos: pb,
                },
            ) => sa == sb && ba == bb && pa == pb,
            (IteratorValue::Reversed { source: sa }, IteratorValue::Reversed { source: sb }) => {
                sa == sb
            }
            (IteratorValue::Repeat { value: va }, IteratorValue::Repeat { value: vb }) => va == vb,
            _ => false,
        }
    }
}

impl Eq for IteratorValue {}

impl std::hash::Hash for IteratorValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            IteratorValue::List { front, back, .. } => {
                front.hash(state);
                back.hash(state);
            }
            IteratorValue::Map { pos, .. } | IteratorValue::Set { pos, .. } => pos.hash(state),
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
            IteratorValue::Str {
                front_pos,
                back_pos,
                ..
            } => {
                front_pos.hash(state);
                back_pos.hash(state);
            }
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
            }
            | IteratorValue::Enumerated {
                source,
                index: remaining,
                ..
            } => {
                source.hash(state);
                remaining.hash(state);
            }
            IteratorValue::Zipped { left, right } => {
                left.hash(state);
                right.hash(state);
            }
            IteratorValue::Chained {
                first,
                second,
                first_done,
            } => {
                first.hash(state);
                second.hash(state);
                first_done.hash(state);
            }
            IteratorValue::Flattened { source, inner } => {
                source.hash(state);
                inner.hash(state);
            }
            IteratorValue::Cycled {
                source,
                buffer,
                buf_pos,
            } => {
                source.hash(state);
                buffer.hash(state);
                buf_pos.hash(state);
            }
            IteratorValue::Reversed { source } => {
                source.hash(state);
            }
            IteratorValue::Repeat { value } => {
                value.hash(state);
            }
        }
    }
}
