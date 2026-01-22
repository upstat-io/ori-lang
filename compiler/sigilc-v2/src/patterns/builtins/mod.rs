//! Built-in pattern definitions.
//!
//! This module contains implementations for all 14 built-in Sigil patterns.

mod run;
mod try_pattern;
mod match_pattern;
mod map;
mod filter;
mod fold;
mod find;
mod collect;
mod recurse;
mod parallel;
mod timeout;
mod retry;
mod cache;
mod validate;

pub use run::RunPattern;
pub use try_pattern::TryPattern;
pub use match_pattern::MatchPattern;
pub use map::MapPattern;
pub use filter::FilterPattern;
pub use fold::FoldPattern;
pub use find::FindPattern;
pub use collect::CollectPattern;
pub use recurse::RecursePattern;
pub use parallel::ParallelPattern;
pub use timeout::TimeoutPattern;
pub use retry::RetryPattern;
pub use cache::CachePattern;
pub use validate::ValidatePattern;

use super::PatternRegistry;

/// Register all built-in patterns with the given registry.
pub fn register_all(registry: &mut PatternRegistry) {
    registry.register(RunPattern);
    registry.register(TryPattern);
    registry.register(MatchPattern);
    registry.register(MapPattern);
    registry.register(FilterPattern);
    registry.register(FoldPattern);
    registry.register(FindPattern);
    registry.register(CollectPattern);
    registry.register(RecursePattern);
    registry.register(ParallelPattern);
    registry.register(TimeoutPattern);
    registry.register(RetryPattern);
    registry.register(CachePattern);
    registry.register(ValidatePattern);
}
