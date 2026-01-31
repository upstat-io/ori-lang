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
mod tests {
    use super::*;
    use crate::PatternDefinition;

    #[test]
    fn test_pattern_names() {
        assert_eq!(PrintPattern.name(), "print");
        assert_eq!(PanicPattern.name(), "panic");
        assert_eq!(CatchPattern.name(), "catch");
        assert_eq!(TodoPattern.name(), "todo");
        assert_eq!(UnreachablePattern.name(), "unreachable");
    }

    #[test]
    fn test_required_props() {
        assert_eq!(PrintPattern.required_props(), &["msg"]);
        assert_eq!(PanicPattern.required_props(), &["msg"]);
        assert_eq!(CatchPattern.required_props(), &["expr"]);
        assert_eq!(TodoPattern.required_props(), &[] as &[&str]);
        assert_eq!(UnreachablePattern.required_props(), &[] as &[&str]);
    }

    #[test]
    fn test_optional_props() {
        assert_eq!(TodoPattern.optional_props(), &["reason"]);
        assert_eq!(UnreachablePattern.optional_props(), &["reason"]);
    }
}
