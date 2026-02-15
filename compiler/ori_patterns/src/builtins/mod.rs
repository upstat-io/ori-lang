//! Core `function_exp` pattern implementations: print, panic, catch, todo, unreachable.

mod catch;
mod panic;
mod print;
mod todo;
mod unreachable;

pub use catch::CatchPattern;
pub use panic::PanicPattern;
pub use print::PrintPattern;
pub use todo::TodoPattern;
pub use unreachable::UnreachablePattern;

#[cfg(test)]
mod tests;
