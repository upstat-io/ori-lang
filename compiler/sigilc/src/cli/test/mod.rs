// Test runner for Sigil
// Public API re-exports

mod coverage;
mod discovery;
mod introspect;
mod paths;
mod result;
mod runner;

pub use coverage::check_coverage;
pub use runner::{test_all, test_file};
