// Built-in pattern definitions for the Sigil compiler
//
// Each pattern is implemented as a struct that implements PatternDefinition.
// This trait provides type checking (infer_type), evaluation (evaluate),
// and documentation (description, help, examples) in a single, self-contained module.

mod fold;
mod map;
mod filter;
mod collect;
mod recurse;
mod iterate;
mod transform;
mod count;
mod parallel;

pub use fold::FoldPattern;
pub use map::MapPattern;
pub use filter::FilterPattern;
pub use collect::CollectPattern;
pub use recurse::RecursePattern;
pub use iterate::IteratePattern;
pub use transform::TransformPattern;
pub use count::CountPattern;
pub use parallel::ParallelPattern;
