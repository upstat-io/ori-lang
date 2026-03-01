//! Size hint computation for iterator variants.

use super::IteratorValue;

impl IteratorValue {
    /// Returns `(lower_bound, Option<upper_bound>)` for remaining items.
    ///
    /// Mirrors Rust's `Iterator::size_hint()` contract:
    /// - `lower` is a guaranteed minimum
    /// - `upper` is `Some(n)` when the exact or maximum count is known
    /// - `None` upper means unbounded or unknown
    pub fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            IteratorValue::List { front, back, .. } => {
                let remaining = back.saturating_sub(*front);
                (remaining, Some(remaining))
            }
            IteratorValue::Set { items, pos } => {
                let remaining = items.len().saturating_sub(*pos);
                (remaining, Some(remaining))
            }
            IteratorValue::Range {
                current,
                end,
                step,
                inclusive,
            } => match end {
                None => (usize::MAX, None),
                Some(end_val) => {
                    let count = super::range_len(*current, *end_val, *step, *inclusive);
                    (count, Some(count))
                }
            },
            IteratorValue::Map { entries, pos } => {
                let remaining = entries.len().saturating_sub(*pos);
                (remaining, Some(remaining))
            }
            IteratorValue::Str {
                front_pos,
                back_pos,
                ..
            } => {
                let remaining_bytes = back_pos.saturating_sub(*front_pos);
                // Each char is 1-4 bytes in UTF-8
                let lower = remaining_bytes.div_ceil(4);
                (lower, Some(remaining_bytes))
            }
            // Reversed/Mapped: 1:1 with source
            IteratorValue::Reversed { source } | IteratorValue::Mapped { source, .. } => {
                source.size_hint()
            }
            IteratorValue::Filtered { source, .. } => {
                // Filter can drop any number of items
                let (_, upper) = source.size_hint();
                (0, upper)
            }
            IteratorValue::TakeN { source, remaining } => {
                let (src_lower, src_upper) = source.size_hint();
                let lower = src_lower.min(*remaining);
                let upper = src_upper.map_or(*remaining, |u| u.min(*remaining));
                (lower, Some(upper))
            }
            IteratorValue::SkipN { source, remaining } => {
                let (src_lower, src_upper) = source.size_hint();
                let lower = src_lower.saturating_sub(*remaining);
                let upper = src_upper.map(|u| u.saturating_sub(*remaining));
                (lower, upper)
            }
            // Enumerated: 1:1 with source
            IteratorValue::Enumerated { source, .. } => source.size_hint(),
            // Zipped: limited by the shorter side
            IteratorValue::Zipped { left, right } => {
                let (l_lo, l_up) = left.size_hint();
                let (r_lo, r_up) = right.size_hint();
                let lower = l_lo.min(r_lo);
                let upper = match (l_up, r_up) {
                    (Some(l), Some(r)) => Some(l.min(r)),
                    (Some(l), None) => Some(l),
                    (None, Some(r)) => Some(r),
                    (None, None) => None,
                };
                (lower, upper)
            }
            // Chained: sum of both sides
            IteratorValue::Chained {
                first,
                second,
                first_done,
            } => {
                if *first_done {
                    return second.size_hint();
                }
                let (f_lo, f_up) = first.size_hint();
                let (s_lo, s_up) = second.size_hint();
                let lower = f_lo.saturating_add(s_lo);
                let upper = match (f_up, s_up) {
                    (Some(f), Some(s)) => f.checked_add(s),
                    _ => None,
                };
                (lower, upper)
            }
            // Flattened: unknowable — items may expand or collapse
            IteratorValue::Flattened { .. } => (0, None),
            // Repeat: always infinite
            IteratorValue::Repeat { .. } => (usize::MAX, None),
            // Cycled: infinite if non-empty buffer, else depends on source state
            IteratorValue::Cycled { source, buffer, .. } => {
                if source.is_none() {
                    if buffer.is_empty() {
                        (0, Some(0))
                    } else {
                        (usize::MAX, None)
                    }
                } else {
                    // Still consuming source — can't know total
                    let (src_lo, _) = source.as_ref().map_or((0, Some(0)), |s| s.size_hint());
                    (src_lo, None)
                }
            }
        }
    }
}
