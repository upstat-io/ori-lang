//! Semantic analysis problem definitions.
//!
//! These problems occur during semantic analysis (name resolution,
//! duplicate definitions, visibility, etc.).

use super::impl_has_span;
use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::ir::Span;
use ori_ir::StringInterner;

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

    /// Convert this problem into a diagnostic.
    ///
    /// The interner parameter is reserved for future Name field lookups.
    #[expect(
        unused_variables,
        reason = "interner reserved for future Name field conversions"
    )]
    pub fn into_diagnostic(&self, interner: &StringInterner) -> Diagnostic {
        match self {
            SemanticProblem::UnknownIdentifier {
                span,
                name,
                similar,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown identifier `{name}`"))
                    .with_label(*span, "not found in this scope");
                if let Some(suggestion) = similar {
                    diag = diag.with_suggestion(format!("try using `{suggestion}`"));
                }
                diag
            }

            SemanticProblem::UnknownFunction {
                span,
                name,
                similar,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown function `@{name}`"))
                    .with_label(*span, "function not found");
                if let Some(suggestion) = similar {
                    diag = diag.with_suggestion(format!("try using `@{suggestion}`"));
                }
                diag
            }

            SemanticProblem::UnknownConfig {
                span,
                name,
                similar,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown config `${name}`"))
                    .with_label(*span, "config not found");
                if let Some(suggestion) = similar {
                    diag = diag.with_suggestion(format!("try using `${suggestion}`"));
                }
                diag
            }

            SemanticProblem::DuplicateDefinition {
                span,
                name,
                kind,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!("duplicate {kind} definition `{name}`"))
                .with_label(*span, "duplicate definition")
                .with_secondary_label(*first_span, "first definition here"),

            SemanticProblem::PrivateAccess { span, name, kind } => {
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("{kind} `{name}` is private"))
                    .with_label(*span, "private, cannot access")
                    .with_suggestion(format!(
                        "add `pub` to the {kind} definition to make it public"
                    ))
            }

            SemanticProblem::ImportNotFound { span, path } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("cannot find module `{path}`"))
                .with_label(*span, "module not found")
                .with_note("check that the file path is correct and the file exists"),

            SemanticProblem::ImportedItemNotFound { span, item, module } => {
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("cannot find `{item}` in module `{module}`"))
                    .with_label(*span, "not found in module")
                    .with_note("check the item is exported from the module")
            }

            SemanticProblem::ImmutableMutation {
                span,
                name,
                binding_span,
            } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("cannot mutate immutable binding `{name}`"))
                .with_label(*span, "cannot mutate")
                .with_secondary_label(*binding_span, "defined as immutable here")
                .with_suggestion("use `let mut` for mutable bindings"),

            SemanticProblem::UseBeforeInit { span, name } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("use of possibly uninitialized `{name}`"))
                .with_label(*span, "used before initialization")
                .with_suggestion("initialize the variable before using it"),

            SemanticProblem::MissingTest { span, func_name } => Diagnostic::error(ErrorCode::E3001)
                .with_message(format!("function `@{func_name}` has no tests"))
                .with_label(*span, "missing test")
                .with_note("every function requires at least one test"),

            SemanticProblem::TestTargetNotFound {
                span,
                test_name,
                target_name,
            } => Diagnostic::error(ErrorCode::E3001)
                .with_message(format!(
                    "test `@{test_name}` targets unknown function `@{target_name}`"
                ))
                .with_label(*span, "function not found")
                .with_note("check the function name in `tests @target_name`"),

            SemanticProblem::BreakOutsideLoop { span } => Diagnostic::error(ErrorCode::E3002)
                .with_message("`break` outside of loop")
                .with_label(
                    *span,
                    "`break` can only appear inside `loop` or `for` bodies",
                )
                .with_suggestion("move this statement inside a loop body"),

            SemanticProblem::ContinueOutsideLoop { span } => Diagnostic::error(ErrorCode::E3002)
                .with_message("`continue` outside of loop")
                .with_label(
                    *span,
                    "`continue` can only appear inside `loop` or `for` bodies",
                )
                .with_suggestion("move this statement inside a loop body"),

            SemanticProblem::SelfOutsideMethod { span } => Diagnostic::error(ErrorCode::E3002)
                .with_message("`self` outside of method")
                .with_label(*span, "`self` is only available in `impl` block methods")
                .with_suggestion("define this function inside an `impl` block"),

            SemanticProblem::InfiniteRecursion { span, func_name } => {
                Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("function `@{func_name}` may recurse infinitely"))
                    .with_label(*span, "unconditional recursion")
                    .with_suggestion("add a base case to stop recursion")
            }

            SemanticProblem::UnusedVariable { span, name } => {
                let mut diag = Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("unused variable `{name}`"))
                    .with_label(*span, "never used");
                if !name.starts_with('_') {
                    diag = diag.with_suggestion(format!("prefix with underscore: `_{name}`"));
                }
                diag
            }

            SemanticProblem::UnusedFunction { span, name } => Diagnostic::warning(ErrorCode::E3003)
                .with_message(format!("unused function `@{name}`"))
                .with_label(*span, "never called")
                .with_suggestion("remove the function or add a call to it"),

            SemanticProblem::UnreachableCode { span } => Diagnostic::warning(ErrorCode::E3003)
                .with_message("unreachable code")
                .with_label(*span, "this code will never execute")
                .with_suggestion("remove this code or restructure the control flow"),

            SemanticProblem::NonExhaustiveMatch {
                span,
                missing_patterns,
            } => {
                let missing = missing_patterns.join(", ");
                Diagnostic::error(ErrorCode::E3002)
                    .with_message("non-exhaustive match")
                    .with_label(*span, "patterns not covered")
                    .with_note(format!("missing patterns: {missing}"))
            }

            SemanticProblem::RedundantPattern {
                span,
                covered_by_span,
            } => Diagnostic::warning(ErrorCode::E3003)
                .with_message("redundant pattern")
                .with_label(*span, "this pattern is unreachable")
                .with_secondary_label(*covered_by_span, "already covered by this pattern"),

            SemanticProblem::MissingCapability { span, capability } => {
                Diagnostic::error(ErrorCode::E3002)
                    .with_message(format!("missing capability `{capability}`"))
                    .with_label(*span, "capability not provided")
                    .with_suggestion(format!(
                        "add `uses {capability}` to function signature or provide with `with...in`"
                    ))
            }

            SemanticProblem::DuplicateCapability {
                span,
                capability,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!("duplicate capability `{capability}`"))
                .with_label(*span, "duplicate")
                .with_secondary_label(*first_span, "first provided here"),
        }
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
