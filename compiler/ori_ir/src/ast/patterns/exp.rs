//! Named Expression Constructs (`function_exp`)
//!
//! Contains patterns like recurse, parallel, spawn, timeout, cache, with.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use super::super::ranges::NamedExprRange;
use crate::{ExprId, Name, Span, Spanned};

/// Named expression for `function_exp`.
///
/// Represents: `name: expr`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NamedExpr {
    pub name: Name,
    pub value: ExprId,
    pub span: Span,
}

impl Spanned for NamedExpr {
    fn span(&self) -> Span {
        self.span
    }
}

/// Kind of `function_exp`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionExpKind {
    // Compiler patterns (require special syntax or static analysis)
    Recurse,
    Parallel,
    Spawn,
    Timeout,
    Cache,
    With,
    // Fundamental built-ins (I/O, control flow, error recovery)
    Print,
    Panic,
    Catch,
}

impl FunctionExpKind {
    pub fn name(self) -> &'static str {
        match self {
            FunctionExpKind::Recurse => "recurse",
            FunctionExpKind::Parallel => "parallel",
            FunctionExpKind::Spawn => "spawn",
            FunctionExpKind::Timeout => "timeout",
            FunctionExpKind::Cache => "cache",
            FunctionExpKind::With => "with",
            FunctionExpKind::Print => "print",
            FunctionExpKind::Panic => "panic",
            FunctionExpKind::Catch => "catch",
        }
    }
}

/// Named expression construct (`function_exp`).
///
/// Contains named expressions (`name: value`).
/// Requires named property syntax - positional not allowed.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionExp {
    pub kind: FunctionExpKind,
    pub props: NamedExprRange,
    pub span: Span,
}

impl Spanned for FunctionExp {
    fn span(&self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_exp_kind_name_all_variants() {
        // Verify all 9 FunctionExpKind variants return correct names
        assert_eq!(FunctionExpKind::Recurse.name(), "recurse");
        assert_eq!(FunctionExpKind::Parallel.name(), "parallel");
        assert_eq!(FunctionExpKind::Spawn.name(), "spawn");
        assert_eq!(FunctionExpKind::Timeout.name(), "timeout");
        assert_eq!(FunctionExpKind::Cache.name(), "cache");
        assert_eq!(FunctionExpKind::With.name(), "with");
        assert_eq!(FunctionExpKind::Print.name(), "print");
        assert_eq!(FunctionExpKind::Panic.name(), "panic");
        assert_eq!(FunctionExpKind::Catch.name(), "catch");
    }

    #[test]
    fn test_function_exp_kind_eq_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(FunctionExpKind::Recurse);
        set.insert(FunctionExpKind::Recurse); // duplicate
        set.insert(FunctionExpKind::Parallel);
        set.insert(FunctionExpKind::Spawn);

        assert_eq!(set.len(), 3);
        assert!(set.contains(&FunctionExpKind::Recurse));
        assert!(set.contains(&FunctionExpKind::Parallel));
        assert!(set.contains(&FunctionExpKind::Spawn));
        assert!(!set.contains(&FunctionExpKind::Cache));
    }

    #[test]
    fn test_function_exp_kind_copy_clone() {
        let kind = FunctionExpKind::Timeout;
        let copied = kind; // Copy
        let cloned = kind.clone(); // Clone

        assert_eq!(kind, copied);
        assert_eq!(kind, cloned);
    }
}
