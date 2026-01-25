//! Core `function_exp` pattern implementations: print and panic.

mod print;
mod panic;

pub use print::PrintPattern;
pub use panic::PanicPattern;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::PatternDefinition;

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
