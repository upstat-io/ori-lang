//! Semantic analysis problem definitions.
//!
//! These problems occur during semantic analysis (name resolution,
//! duplicate definitions, visibility, etc.).

use super::impl_has_span;
use crate::ir::Span;

/// Problems that occur during semantic analysis.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Hash, Debug for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum SemanticProblem {
    /// Unknown identifier (not in scope).
    UnknownIdentifier {
        span: Span,
        name: String,
        /// Similar name if found (for "did you mean?").
        similar: Option<String>,
    },

    /// Unknown function reference.
    UnknownFunction {
        span: Span,
        name: String,
        similar: Option<String>,
    },

    /// Unknown config variable.
    UnknownConfig {
        span: Span,
        name: String,
        /// Similar config name if found (for "did you mean?").
        similar: Option<String>,
    },

    /// Duplicate definition.
    DuplicateDefinition {
        span: Span,
        name: String,
        kind: DefinitionKind,
        first_span: Span,
    },

    /// Accessing private item.
    PrivateAccess {
        span: Span,
        name: String,
        kind: DefinitionKind,
    },

    /// Import not found.
    ImportNotFound { span: Span, path: String },

    /// Imported item not found in module.
    ImportedItemNotFound {
        span: Span,
        item: String,
        module: String,
    },

    /// Mutating immutable binding.
    ImmutableMutation {
        span: Span,
        name: String,
        binding_span: Span,
    },

    /// Using uninitialized variable.
    UseBeforeInit { span: Span, name: String },

    /// Function missing required test.
    MissingTest { span: Span, func_name: String },

    /// Test targets unknown function.
    TestTargetNotFound {
        span: Span,
        test_name: String,
        target_name: String,
    },

    /// Break outside loop.
    BreakOutsideLoop { span: Span },

    /// Continue outside loop.
    ContinueOutsideLoop { span: Span },

    /// Return outside function.
    ReturnOutsideFunction { span: Span },

    /// Self reference outside method.
    SelfOutsideMethod { span: Span },

    /// Recursive function without base case.
    InfiniteRecursion { span: Span, func_name: String },

    /// Unused variable warning.
    UnusedVariable { span: Span, name: String },

    /// Unused function warning.
    UnusedFunction { span: Span, name: String },

    /// Unreachable code warning.
    UnreachableCode { span: Span },

    /// Pattern matching is not exhaustive.
    NonExhaustiveMatch {
        span: Span,
        missing_patterns: Vec<String>,
    },

    /// Redundant pattern arm (already covered).
    RedundantPattern { span: Span, covered_by_span: Span },

    /// Capability not provided.
    MissingCapability { span: Span, capability: String },

    /// Capability already provided.
    DuplicateCapability {
        span: Span,
        capability: String,
        first_span: Span,
    },
}

/// Kind of definition for duplicate/private access errors.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum DefinitionKind {
    Function,
    Variable,
    Config,
    Type,
    Test,
    Import,
}

impl DefinitionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DefinitionKind::Function => "function",
            DefinitionKind::Variable => "variable",
            DefinitionKind::Config => "config",
            DefinitionKind::Type => "type",
            DefinitionKind::Test => "test",
            DefinitionKind::Import => "import",
        }
    }
}

impl std::fmt::Display for DefinitionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// Generate HasSpan implementation using macro.
// All variants use the standard `span` field.
impl_has_span! {
    SemanticProblem {
        span: [
            UnknownIdentifier,
            UnknownFunction,
            UnknownConfig,
            DuplicateDefinition,
            PrivateAccess,
            ImportNotFound,
            ImportedItemNotFound,
            ImmutableMutation,
            UseBeforeInit,
            MissingTest,
            TestTargetNotFound,
            BreakOutsideLoop,
            ContinueOutsideLoop,
            ReturnOutsideFunction,
            SelfOutsideMethod,
            InfiniteRecursion,
            UnusedVariable,
            UnusedFunction,
            UnreachableCode,
            NonExhaustiveMatch,
            RedundantPattern,
            MissingCapability,
            DuplicateCapability,
        ],
    }
}

impl SemanticProblem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        <Self as super::HasSpan>::span(self)
    }

    /// Check if this is a warning (vs error).
    ///
    /// Note: This method is kept manual because the warning logic
    /// is different from the span extraction pattern.
    pub fn is_warning(&self) -> bool {
        matches!(
            self,
            SemanticProblem::UnusedVariable { .. }
                | SemanticProblem::UnusedFunction { .. }
                | SemanticProblem::UnreachableCode { .. }
                | SemanticProblem::RedundantPattern { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_identifier() {
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        assert_eq!(problem.span(), Span::new(20, 25));
        assert!(!problem.is_warning());
    }

    #[test]
    fn test_duplicate_definition() {
        let problem = SemanticProblem::DuplicateDefinition {
            span: Span::new(100, 110),
            name: "bar".into(),
            kind: DefinitionKind::Function,
            first_span: Span::new(10, 20),
        };

        assert_eq!(problem.span(), Span::new(100, 110));
        assert!(!problem.is_warning());
    }

    #[test]
    fn test_unused_variable() {
        let problem = SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        };

        assert_eq!(problem.span(), Span::new(5, 10));
        assert!(problem.is_warning());
    }

    #[test]
    fn test_non_exhaustive_match() {
        let problem = SemanticProblem::NonExhaustiveMatch {
            span: Span::new(0, 50),
            missing_patterns: vec!["None".into(), "Some(Err(_))".into()],
        };

        assert_eq!(problem.span(), Span::new(0, 50));
        assert!(!problem.is_warning());
    }

    #[test]
    fn test_definition_kind_display() {
        assert_eq!(DefinitionKind::Function.to_string(), "function");
        assert_eq!(DefinitionKind::Variable.to_string(), "variable");
        assert_eq!(DefinitionKind::Config.to_string(), "config");
        assert_eq!(DefinitionKind::Type.to_string(), "type");
    }

    #[test]
    fn test_problem_equality() {
        let p1 = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        let p2 = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        let p3 = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "bar".into(),
            similar: None,
        };

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_problem_hash() {
        use std::collections::HashSet;

        let p1 = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        let p2 = p1.clone();
        let p3 = SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        };

        let mut set = HashSet::new();
        set.insert(p1);
        set.insert(p2); // duplicate
        set.insert(p3);

        assert_eq!(set.len(), 2);
    }
}
