//! Semantic analysis problem definitions.
//!
//! These problems occur during semantic analysis (name resolution,
//! duplicate definitions, visibility, etc.).
//!
//! # Production Coverage
//!
//! Most variants are reserved and have no production producer. Their
//! `into_diagnostic()` rendering remains covered independently.
//!
//! Production producers emit:
//! - `MissingTest` — `commands/check.rs` (test coverage analysis)
//! - `NonExhaustiveMatch` — via `pattern_problem_to_diagnostic()`
//! - `RedundantPattern` — via `pattern_problem_to_diagnostic()`

use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::ir::{Name, Span, StringInterner};

/// Problems that occur during semantic analysis.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Hash, Debug for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum SemanticProblem {
    /// Unknown identifier (not in scope).
    UnknownIdentifier {
        span: Span,
        name: Name,
        /// Similar name if found (for "did you mean?").
        similar: Option<Name>,
    },

    /// Unknown function reference.
    UnknownFunction {
        span: Span,
        name: Name,
        similar: Option<Name>,
    },

    /// Unknown config variable.
    UnknownConfig {
        span: Span,
        name: Name,
        /// Similar config name if found (for "did you mean?").
        similar: Option<Name>,
    },

    /// Duplicate definition.
    DuplicateDefinition {
        span: Span,
        name: Name,
        kind: DefinitionKind,
        first_span: Span,
    },

    /// Accessing private item.
    PrivateAccess {
        span: Span,
        name: Name,
        kind: DefinitionKind,
    },

    /// Import not found.
    ImportNotFound {
        span: Span,
        /// File path — not an identifier, stays as `String`.
        path: String,
    },

    /// Imported item not found in module.
    ImportedItemNotFound {
        span: Span,
        item: Name,
        /// Module path — not an identifier, stays as `String`.
        module: String,
    },

    /// Mutating immutable binding.
    ImmutableMutation {
        span: Span,
        name: Name,
        binding_span: Span,
    },

    /// Using uninitialized variable.
    UseBeforeInit { span: Span, name: Name },

    /// Function missing required test.
    MissingTest { span: Span, func_name: Name },

    /// Test targets unknown function.
    TestTargetNotFound {
        span: Span,
        test_name: Name,
        target_name: Name,
    },

    /// Break outside loop.
    BreakOutsideLoop { span: Span },

    /// Continue outside loop.
    ContinueOutsideLoop { span: Span },

    /// Self reference outside method.
    SelfOutsideMethod { span: Span },

    /// Recursive function without base case.
    InfiniteRecursion { span: Span, func_name: Name },

    /// Unused variable warning.
    UnusedVariable { span: Span, name: Name },

    /// Unused function warning.
    UnusedFunction { span: Span, name: Name },

    /// Unreachable code warning.
    UnreachableCode { span: Span },

    /// Pattern matching is not exhaustive.
    NonExhaustiveMatch {
        span: Span,
        /// Pattern descriptions — not identifiers, stays as `Vec<String>`.
        missing_patterns: Vec<String>,
    },

    /// Redundant pattern arm (already covered).
    RedundantPattern { span: Span, covered_by_span: Span },

    /// Capability not provided.
    MissingCapability { span: Span, capability: Name },

    /// Capability already provided.
    DuplicateCapability {
        span: Span,
        capability: Name,
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

impl SemanticProblem {
    /// Get the primary span of this problem.
    ///
    /// All variants carry a `span` field as their primary source location.
    pub fn span(&self) -> Span {
        match self {
            SemanticProblem::UnknownIdentifier { span, .. }
            | SemanticProblem::UnknownFunction { span, .. }
            | SemanticProblem::UnknownConfig { span, .. }
            | SemanticProblem::DuplicateDefinition { span, .. }
            | SemanticProblem::PrivateAccess { span, .. }
            | SemanticProblem::ImportNotFound { span, .. }
            | SemanticProblem::ImportedItemNotFound { span, .. }
            | SemanticProblem::ImmutableMutation { span, .. }
            | SemanticProblem::UseBeforeInit { span, .. }
            | SemanticProblem::MissingTest { span, .. }
            | SemanticProblem::TestTargetNotFound { span, .. }
            | SemanticProblem::BreakOutsideLoop { span }
            | SemanticProblem::ContinueOutsideLoop { span }
            | SemanticProblem::SelfOutsideMethod { span }
            | SemanticProblem::InfiniteRecursion { span, .. }
            | SemanticProblem::UnusedVariable { span, .. }
            | SemanticProblem::UnusedFunction { span, .. }
            | SemanticProblem::UnreachableCode { span }
            | SemanticProblem::NonExhaustiveMatch { span, .. }
            | SemanticProblem::RedundantPattern { span, .. }
            | SemanticProblem::MissingCapability { span, .. }
            | SemanticProblem::DuplicateCapability { span, .. } => *span,
        }
    }

    /// Classify unused declarations, unreachable code, and redundant patterns
    /// as warnings.
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
    /// Uses the interner to resolve interned `Name` fields to display strings.
    #[cold]
    pub fn into_diagnostic(&self, interner: &StringInterner) -> Diagnostic {
        if let Some(diagnostic) = self.name_resolution_diagnostic(interner) {
            return diagnostic;
        }
        if let Some(diagnostic) = self.declaration_diagnostic(interner) {
            return diagnostic;
        }
        match self {
            SemanticProblem::InfiniteRecursion { span, func_name } => {
                let func_name = interner.lookup(*func_name);
                Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("function `@{func_name}` may recurse infinitely"))
                    .with_label(*span, "unconditional recursion")
                    .with_suggestion("add a base case to stop recursion")
            }

            SemanticProblem::UnusedVariable { span, name } => {
                let name = interner.lookup(*name);
                let mut diag = Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("unused variable `{name}`"))
                    .with_label(*span, "never used");
                if !name.starts_with('_') {
                    diag = diag.with_suggestion(format!("prefix with underscore: `_{name}`"));
                }
                diag
            }

            SemanticProblem::UnusedFunction { span, name } => {
                let name = interner.lookup(*name);
                Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("unused function `@{name}`"))
                    .with_label(*span, "never called")
                    .with_suggestion("remove the function or add a call to it")
            }

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
                let capability = interner.lookup(*capability);
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
            } => {
                let capability = interner.lookup(*capability);
                Diagnostic::error(ErrorCode::E2006)
                    .with_message(format!("duplicate capability `{capability}`"))
                    .with_label(*span, "duplicate")
                    .with_secondary_label(*first_span, "first provided here")
            }
            SemanticProblem::UnknownIdentifier { .. }
            | SemanticProblem::UnknownFunction { .. }
            | SemanticProblem::UnknownConfig { .. }
            | SemanticProblem::DuplicateDefinition { .. }
            | SemanticProblem::PrivateAccess { .. }
            | SemanticProblem::ImportNotFound { .. }
            | SemanticProblem::ImportedItemNotFound { .. }
            | SemanticProblem::ImmutableMutation { .. }
            | SemanticProblem::UseBeforeInit { .. }
            | SemanticProblem::MissingTest { .. }
            | SemanticProblem::TestTargetNotFound { .. }
            | SemanticProblem::BreakOutsideLoop { .. }
            | SemanticProblem::ContinueOutsideLoop { .. }
            | SemanticProblem::SelfOutsideMethod { .. } => {
                unreachable!("handled by name-resolution diagnostic helper")
            }
        }
    }

    fn declaration_diagnostic(&self, interner: &StringInterner) -> Option<Diagnostic> {
        let diagnostic = match self {
            SemanticProblem::ImmutableMutation {
                span,
                name,
                binding_span,
            } => Diagnostic::error(ErrorCode::E2039)
                .with_message(format!(
                    "cannot mutate immutable binding `{}`",
                    interner.lookup(*name)
                ))
                .with_label(*span, "cannot mutate")
                .with_secondary_label(*binding_span, "defined as immutable here")
                .with_suggestion("remove the `$` prefix to make this binding mutable"),
            SemanticProblem::UseBeforeInit { span, name } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!(
                    "use of possibly uninitialized `{}`",
                    interner.lookup(*name)
                ))
                .with_label(*span, "used before initialization")
                .with_suggestion("initialize the variable before using it"),
            SemanticProblem::MissingTest { span, func_name } => {
                let name = interner.lookup(*func_name);
                Diagnostic::error(ErrorCode::E3010)
                    .with_message(format!("function `@{name}` has no tests"))
                    .with_label(*span, "missing test")
                    .with_note(format!(
                        "add a test with `@test_{name} tests @{name} () -> void`"
                    ))
            }
            SemanticProblem::TestTargetNotFound {
                span,
                test_name,
                target_name,
            } => Diagnostic::error(ErrorCode::E3011)
                .with_message(format!(
                    "test `@{}` targets unknown function `@{}`",
                    interner.lookup(*test_name),
                    interner.lookup(*target_name)
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
            _ => return None,
        };
        Some(diagnostic)
    }

    fn name_resolution_diagnostic(&self, interner: &StringInterner) -> Option<Diagnostic> {
        let diagnostic = match self {
            SemanticProblem::UnknownIdentifier {
                span,
                name,
                similar,
            } => unknown_name_diagnostic(
                interner,
                *span,
                *name,
                *similar,
                "identifier",
                "",
                "not found in this scope",
            ),
            SemanticProblem::UnknownFunction {
                span,
                name,
                similar,
            } => unknown_name_diagnostic(
                interner,
                *span,
                *name,
                *similar,
                "function",
                "@",
                "function not found",
            ),
            SemanticProblem::UnknownConfig {
                span,
                name,
                similar,
            } => unknown_name_diagnostic(
                interner,
                *span,
                *name,
                *similar,
                "config",
                "$",
                "config not found",
            ),
            SemanticProblem::DuplicateDefinition {
                span,
                name,
                kind,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!(
                    "duplicate {kind} definition `{}`",
                    interner.lookup(*name)
                ))
                .with_label(*span, "duplicate definition")
                .with_secondary_label(*first_span, "first definition here"),
            SemanticProblem::PrivateAccess { span, name, kind } => {
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("{kind} `{}` is private", interner.lookup(*name)))
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
                    .with_message(format!(
                        "cannot find `{}` in module `{module}`",
                        interner.lookup(*item)
                    ))
                    .with_label(*span, "not found in module")
                    .with_note("check the item is exported from the module")
            }
            _ => return None,
        };
        Some(diagnostic)
    }
}

fn unknown_name_diagnostic(
    interner: &StringInterner,
    span: Span,
    name: Name,
    similar: Option<Name>,
    kind: &str,
    sigil: &str,
    label: &str,
) -> Diagnostic {
    let name = interner.lookup(name);
    let mut diagnostic = Diagnostic::error(ErrorCode::E2003)
        .with_message(format!("unknown {kind} `{sigil}{name}`"))
        .with_label(span, label);
    if let Some(similar) = similar {
        diagnostic =
            diagnostic.with_suggestion(format!("try using `{sigil}{}`", interner.lookup(similar)));
    }
    diagnostic
}

mod test_coverage;

pub use test_coverage::{
    check_test_coverage, const_eval_problem_to_diagnostic, const_eval_problems_summary,
    const_eval_problems_to_diagnostics, pattern_problem_to_diagnostic,
};

#[cfg(test)]
mod tests;
