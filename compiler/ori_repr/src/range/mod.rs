//! Value range analysis — interval lattice and supporting types.
//!
//! Implements abstract interpretation over integer intervals for the
//! representation optimization pipeline. Each `ValueRange` represents
//! a set of possible `i64` values for a variable or expression.
//!
//! # Architecture
//!
//! - `ValueRange` — the core lattice: `Bottom ⊑ Bounded ⊑ Top`
//! - `IntWidth::signed_range()` / `unsigned_range()` — width bounds
//! - `RangeAnalysisConfig` — budget limits for fixpoint iteration
//!
//! # Phase
//!
//! Range analysis runs after ARC lowering (on `ArcFunction`) and before
//! LLVM codegen. Results are stored in `ReprPlan::function_var_ranges`
//! and consumed by §04 (integer narrowing) and §05 (float narrowing).

use ori_types::{Idx, Pool, Tag};

use crate::repr::IntWidth;

pub mod conditional;
pub mod field_summary;
pub mod transfer;

/// A closed interval [lo, hi] over i64 values.
///
/// Invariant: `lo <= hi` for `Bounded` (empty sets are `Bottom`).
///
/// Forms a lattice:
/// - `Bottom` — no possible values (unreachable code)
/// - `Bounded { lo, hi }` — exactly the values in [lo, hi]
/// - `Top` — all possible i64 values (analysis gave up)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueRange {
    /// No possible values (unreachable code).
    Bottom,
    /// Exactly the values in [lo, hi] (inclusive).
    Bounded { lo: i64, hi: i64 },
    /// All possible i64 values (analysis gave up or unconstrained).
    Top,
}

/// Default is `Top` — the safe conservative "we don't know anything" value.
///
/// This is load-bearing: `ReprPlan::var_range()` uses `.unwrap_or_default()`
/// so unanalyzed variables get `Top` (not `Bottom`).
impl Default for ValueRange {
    fn default() -> Self {
        Self::Top
    }
}

impl ValueRange {
    /// Lattice join (least upper bound — union of possible values).
    ///
    /// `join(a, b)` returns the smallest `ValueRange` that contains
    /// all values from both `a` and `b`.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, x) | (x, Self::Bottom) => x,
            (Self::Top, _) | (_, Self::Top) => Self::Top,
            (Self::Bounded { lo: a_lo, hi: a_hi }, Self::Bounded { lo: b_lo, hi: b_hi }) => {
                Self::Bounded {
                    lo: a_lo.min(b_lo),
                    hi: a_hi.max(b_hi),
                }
            }
        }
    }

    /// Lattice meet (greatest lower bound — intersection of possible values).
    ///
    /// `meet(a, b)` returns the largest `ValueRange` contained in both
    /// `a` and `b`. If no values are in common, returns `Bottom`.
    #[must_use]
    pub fn meet(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            (Self::Top, x) | (x, Self::Top) => x,
            (Self::Bounded { lo: a_lo, hi: a_hi }, Self::Bounded { lo: b_lo, hi: b_hi }) => {
                let lo = a_lo.max(b_lo);
                let hi = a_hi.min(b_hi);
                if lo > hi {
                    Self::Bottom
                } else {
                    Self::Bounded { lo, hi }
                }
            }
        }
    }

    /// Does this range fit within the given signed integer width?
    ///
    /// `Bottom` fits in all widths (no values to overflow).
    /// `Top` fits only in `I64` (full range).
    #[must_use]
    pub fn fits_in(&self, width: IntWidth) -> bool {
        match self {
            Self::Bottom => true,
            Self::Top => width == IntWidth::I64,
            Self::Bounded { lo, hi } => {
                let w = width.signed_range();
                match w {
                    Self::Top => true,
                    Self::Bounded { lo: w_lo, hi: w_hi } => *lo >= w_lo && *hi <= w_hi,
                    Self::Bottom => false,
                }
            }
        }
    }

    /// Minimum signed integer width needed to represent this range.
    ///
    /// `Bottom` → `I8` (smallest valid width).
    /// `Top` → `I64` (full range needed).
    #[must_use]
    pub fn min_width(&self) -> IntWidth {
        match self {
            Self::Bottom => IntWidth::I8,
            Self::Top => IntWidth::I64,
            Self::Bounded { .. } => {
                if self.fits_in(IntWidth::I8) {
                    IntWidth::I8
                } else if self.fits_in(IntWidth::I16) {
                    IntWidth::I16
                } else if self.fits_in(IntWidth::I32) {
                    IntWidth::I32
                } else {
                    IntWidth::I64
                }
            }
        }
    }

    /// If this range is a single constant value, return it.
    #[must_use]
    pub fn is_constant(&self) -> Option<i64> {
        match self {
            Self::Bounded { lo, hi } if lo == hi => Some(*lo),
            _ => None,
        }
    }

    /// Does this range overlap with another?
    ///
    /// Two ranges overlap if they share at least one value.
    /// `Bottom` overlaps with nothing (not even itself).
    /// `Top` overlaps with everything except `Bottom`.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        // meet returns Bottom iff the ranges are disjoint.
        // But Bottom.meet(anything) = Bottom, which is correct:
        // Bottom has no values, so it can't overlap.
        !matches!(self.meet(*other), Self::Bottom)
    }
}

impl IntWidth {
    /// The range of values representable by this signed integer width.
    ///
    /// `I64` returns `Top` since it covers all `i64` values.
    #[must_use]
    pub fn signed_range(self) -> ValueRange {
        match self {
            Self::I8 => ValueRange::Bounded { lo: -128, hi: 127 },
            Self::I16 => ValueRange::Bounded {
                lo: -32768,
                hi: 32767,
            },
            Self::I32 => ValueRange::Bounded {
                lo: -2_147_483_648,
                hi: 2_147_483_647,
            },
            Self::I64 => ValueRange::Top,
        }
    }

    /// The range of values representable by this unsigned integer width.
    ///
    /// `I64` returns `Top` since it covers all non-negative `i64` values
    /// (and Ori's `int` is always signed, so unsigned I64 = all values).
    #[must_use]
    pub fn unsigned_range(self) -> ValueRange {
        match self {
            Self::I8 => ValueRange::Bounded { lo: 0, hi: 255 },
            Self::I16 => ValueRange::Bounded { lo: 0, hi: 65535 },
            Self::I32 => ValueRange::Bounded {
                lo: 0,
                hi: 4_294_967_295,
            },
            Self::I64 => ValueRange::Top,
        }
    }
}

/// Configuration for range analysis passes.
///
/// Controls budget limits for fixpoint iteration to prevent
/// quadratic blowup on large functions or recursive call graphs.
#[derive(Debug, Clone)]
pub struct RangeAnalysisConfig {
    /// Maximum fixpoint iterations per function (intraprocedural).
    pub max_iterations: usize,
    /// Skip range analysis entirely for functions with more blocks.
    pub max_blocks: usize,
    /// Maximum fixpoint iterations per SCC (interprocedural).
    pub max_scc_iterations: usize,
    /// Maximum total SCC iterations across all SCCs.
    pub max_total_scc_iterations: usize,
}

impl Default for RangeAnalysisConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            max_blocks: 500,
            max_scc_iterations: 10,
            max_total_scc_iterations: 50,
        }
    }
}

/// Check if a type is integer-typed (the only type range analysis tracks).
///
/// Returns `false` for `Idx::ERROR`, newtypes that resolve to non-int,
/// and all non-int types. Handles newtypes by resolving through `Named`
/// and `Alias` tags.
#[must_use]
pub fn is_int_typed(ty: Idx, pool: &Pool) -> bool {
    if ty == Idx::ERROR {
        return false;
    }
    match pool.tag(ty) {
        Tag::Int => true,
        // Newtypes resolve through Named/Alias to their inner type.
        Tag::Named | Tag::Alias => {
            let resolved = pool.resolve_fully(ty);
            resolved != ty && is_int_typed(resolved, pool)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
