//! Core `function_exp` pattern implementations: print, panic, and catch.

mod catch;
mod panic;
mod print;

pub use catch::CatchPattern;
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
        assert_eq!(CatchPattern.name(), "catch");
    }

    #[test]
    fn test_required_props() {
        assert_eq!(PrintPattern.required_props(), &["msg"]);
        assert_eq!(PanicPattern.required_props(), &["msg"]);
        assert_eq!(CatchPattern.required_props(), &["expr"]);
    }
}
