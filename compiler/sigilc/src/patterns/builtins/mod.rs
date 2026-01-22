// Built-in pattern definitions for the Sigil compiler
//
// Each pattern is implemented as a struct that implements PatternDefinition.
// This trait provides type checking (infer_type), evaluation (evaluate),
// and documentation (description, help, examples) in a single, self-contained module.

mod collect;
mod count;
mod filter;
mod find;
mod fold;
mod iterate;
mod map;
mod parallel;
mod recurse;
mod retry;
mod transform;
mod try_pattern;
mod validate;

pub use collect::CollectPattern;
pub use count::CountPattern;
pub use filter::FilterPattern;
pub use find::FindPattern;
pub use fold::FoldPattern;
pub use iterate::IteratePattern;
pub use map::MapPattern;
pub use parallel::ParallelPattern;
pub use recurse::RecursePattern;
pub use retry::RetryPattern;
pub use transform::TransformPattern;
pub use try_pattern::TryPattern;
pub use validate::ValidatePattern;
