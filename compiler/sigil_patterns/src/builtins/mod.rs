//! Core function_exp pattern implementations.
//!
//! Per "Lean Core, Rich Libraries", most builtin patterns have been moved to
//! stdlib methods/functions. Only Print and Panic remain as compiler built-ins.

mod panic;
mod print;

pub use panic::PanicPattern;
pub use print::PrintPattern;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PatternDefinition;

    #[test]
    fn test_pattern_names() {
        assert_eq!(PrintPattern.name(), "print");
        assert_eq!(PanicPattern.name(), "panic");
    }

    #[test]
    fn test_required_props() {
        assert_eq!(PrintPattern.required_props(), &["msg"]);
        assert_eq!(PanicPattern.required_props(), &["msg"]);
    }
}
