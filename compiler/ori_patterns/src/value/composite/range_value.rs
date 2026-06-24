//! Range value type: `RangeValue`.

/// Range value.
///
/// Supports both bounded (`0..10`) and unbounded (`0..`) ranges.
/// When `end` is `None`, the range is infinite in the step direction.
#[derive(Clone, Debug)]
pub struct RangeValue {
    /// Start of range (inclusive).
    pub start: i64,
    /// End of range, or `None` for unbounded ranges.
    pub end: Option<i64>,
    /// Step increment (default 1). Can be negative for descending ranges.
    pub step: i64,
    /// Whether end is inclusive.
    pub inclusive: bool,
}

impl RangeValue {
    /// Create an exclusive range with step 1.
    pub fn exclusive(start: i64, end: i64) -> Self {
        RangeValue {
            start,
            end: Some(end),
            step: 1,
            inclusive: false,
        }
    }

    /// Create an inclusive range with step 1.
    pub fn inclusive(start: i64, end: i64) -> Self {
        RangeValue {
            start,
            end: Some(end),
            step: 1,
            inclusive: true,
        }
    }

    /// Create an exclusive range with custom step.
    pub fn exclusive_with_step(start: i64, end: i64, step: i64) -> Self {
        RangeValue {
            start,
            end: Some(end),
            step,
            inclusive: false,
        }
    }

    /// Create an inclusive range with custom step.
    pub fn inclusive_with_step(start: i64, end: i64, step: i64) -> Self {
        RangeValue {
            start,
            end: Some(end),
            step,
            inclusive: true,
        }
    }

    /// Create an unbounded range with step 1 (`start..`).
    pub fn unbounded(start: i64) -> Self {
        RangeValue {
            start,
            end: None,
            step: 1,
            inclusive: false,
        }
    }

    /// Create an unbounded range with custom step (`start.. by step`).
    pub fn unbounded_with_step(start: i64, step: i64) -> Self {
        RangeValue {
            start,
            end: None,
            step,
            inclusive: false,
        }
    }

    /// Returns `true` if this range is unbounded (has no end).
    pub fn is_unbounded(&self) -> bool {
        self.end.is_none()
    }

    /// Iterate over the range values.
    ///
    /// For unbounded ranges, produces an infinite sequence.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    pub fn iter(&self) -> impl Iterator<Item = i64> {
        let start = self.start;
        let end = self.end;
        let step = self.step;
        let inclusive = self.inclusive;

        // For unbounded ranges, the initial value is always valid (if step != 0)
        let initial = match end {
            None => {
                if step != 0 {
                    Some(start)
                } else {
                    None
                }
            }
            Some(end_val) => match step.cmp(&0) {
                std::cmp::Ordering::Greater => {
                    if inclusive {
                        if start <= end_val {
                            Some(start)
                        } else {
                            None
                        }
                    } else if start < end_val {
                        Some(start)
                    } else {
                        None
                    }
                }
                std::cmp::Ordering::Less => {
                    if inclusive {
                        if start >= end_val {
                            Some(start)
                        } else {
                            None
                        }
                    } else if start > end_val {
                        Some(start)
                    } else {
                        None
                    }
                }
                std::cmp::Ordering::Equal => None,
            },
        };

        std::iter::successors(initial, move |&current| {
            let next = current + step;
            match end {
                // Unbounded: always yield next (infinite)
                None => Some(next),
                Some(end_val) => match step.cmp(&0) {
                    std::cmp::Ordering::Greater => {
                        if inclusive {
                            if next <= end_val {
                                Some(next)
                            } else {
                                None
                            }
                        } else if next < end_val {
                            Some(next)
                        } else {
                            None
                        }
                    }
                    std::cmp::Ordering::Less => {
                        if inclusive {
                            if next >= end_val {
                                Some(next)
                            } else {
                                None
                            }
                        } else if next > end_val {
                            Some(next)
                        } else {
                            None
                        }
                    }
                    std::cmp::Ordering::Equal => None,
                },
            }
        })
    }

    /// Get the length of the range.
    ///
    /// Returns `usize::MAX` for unbounded ranges.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    pub fn len(&self) -> usize {
        if self.step == 0 {
            return 0;
        }

        let Some(end) = self.end else {
            return usize::MAX;
        };

        let adjusted_end = if self.inclusive {
            if self.step > 0 {
                end + 1
            } else {
                end - 1
            }
        } else {
            end
        };

        let diff = if self.step > 0 {
            (adjusted_end - self.start).max(0)
        } else {
            (self.start - adjusted_end).max(0)
        };

        let step_abs = self.step.abs();
        let count = (diff + step_abs - 1) / step_abs; // ceiling division
        usize::try_from(count).unwrap_or(usize::MAX)
    }

    /// Check if the range is empty.
    ///
    /// Unbounded ranges are never empty (unless step is 0).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a value is contained in the range.
    ///
    /// For unbounded ranges, checks only the lower bound and step alignment.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    pub fn contains(&self, value: i64) -> bool {
        // Check bounds
        let in_bounds = match self.step.cmp(&0) {
            std::cmp::Ordering::Greater => {
                let lower_ok = value >= self.start;
                let upper_ok = match self.end {
                    None => true,
                    Some(end) if self.inclusive => value <= end,
                    Some(end) => value < end,
                };
                lower_ok && upper_ok
            }
            std::cmp::Ordering::Less => {
                let lower_ok = value <= self.start;
                let upper_ok = match self.end {
                    None => true,
                    Some(end) if self.inclusive => value >= end,
                    Some(end) => value > end,
                };
                lower_ok && upper_ok
            }
            std::cmp::Ordering::Equal => return false,
        };

        if !in_bounds {
            return false;
        }

        // Check alignment with step
        (value - self.start) % self.step == 0
    }
}
