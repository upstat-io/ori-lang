//! Fixed-point value range analysis.

mod analysis;
mod iteration;
mod narrowing;
mod terminator;
mod widen;

pub(super) use super::ValueRange;
pub(crate) use analysis::range_fixpoint;
pub(super) use analysis::FixpointContext;
pub use analysis::RangeFixpointResult;
pub use widen::{narrow, widen, widen_with_thresholds};

#[cfg(test)]
use super::RangeAnalysisConfig;
#[cfg(test)]
use ori_arc::ArcVarId;

#[cfg(test)]
mod tests;
