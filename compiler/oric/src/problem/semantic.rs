//! Semantic analysis problem definitions.
//!
//! These problems occur during semantic analysis (name resolution,
//! duplicate definitions, visibility, etc.).
//!
//! # Active vs Reserved Variants
//!
//! Most variants are **reserved for a future dedicated semantic analysis pass**
//! and have no production producer yet. Their `into_diagnostic()` rendering is
//! implemented and tested so diagnostics are ready when the pass lands.
//!
//! Currently produced in production code:
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
    /// Uses the interner to resolve interned `Name` fields to display strings.
    #[cold]
    pub fn into_diagnostic(&self, interner: &StringInterner) -> Diagnostic {
        match self {
            SemanticProblem::UnknownIdentifier {
                span,
                name,
                similar,
            } => {
                let name = interner.lookup(*name);
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown identifier `{name}`"))
                    .with_label(*span, "not found in this scope");
                if let Some(s) = similar {
                    let s = interner.lookup(*s);
                    diag = diag.with_suggestion(format!("try using `{s}`"));
                }
                diag
            }

            SemanticProblem::UnknownFunction {
                span,
                name,
                similar,
            } => {
                let name = interner.lookup(*name);
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown function `@{name}`"))
                    .with_label(*span, "function not found");
                if let Some(s) = similar {
                    let s = interner.lookup(*s);
                    diag = diag.with_suggestion(format!("try using `@{s}`"));
                }
                diag
            }

            SemanticProblem::UnknownConfig {
                span,
                name,
                similar,
            } => {
                let name = interner.lookup(*name);
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown config `${name}`"))
                    .with_label(*span, "config not found");
                if let Some(s) = similar {
                    let s = interner.lookup(*s);
                    diag = diag.with_suggestion(format!("try using `${s}`"));
                }
                diag
            }

            SemanticProblem::DuplicateDefinition {
                span,
                name,
                kind,
                first_span,
            } => {
                let name = interner.lookup(*name);
                Diagnostic::error(ErrorCode::E2006)
                    .with_message(format!("duplicate {kind} definition `{name}`"))
                    .with_label(*span, "duplicate definition")
                    .with_secondary_label(*first_span, "first definition here")
            }

            SemanticProblem::PrivateAccess { span, name, kind } => {
                let name = interner.lookup(*name);
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
                let item = interner.lookup(*item);
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("cannot find `{item}` in module `{module}`"))
                    .with_label(*span, "not found in module")
                    .with_note("check the item is exported from the module")
            }

            SemanticProblem::ImmutableMutation {
                span,
                name,
                binding_span,
            } => {
                let name = interner.lookup(*name);
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("cannot mutate immutable binding `{name}`"))
                    .with_label(*span, "cannot mutate")
                    .with_secondary_label(*binding_span, "defined as immutable here")
                    .with_suggestion("use `let mut` for mutable bindings")
            }

            SemanticProblem::UseBeforeInit { span, name } => {
                let name = interner.lookup(*name);
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("use of possibly uninitialized `{name}`"))
                    .with_label(*span, "used before initialization")
                    .with_suggestion("initialize the variable before using it")
            }

            SemanticProblem::MissingTest { span, func_name } => {
                let func_name = interner.lookup(*func_name);
                Diagnostic::error(ErrorCode::E3001)
                    .with_message(format!("function `@{func_name}` has no tests"))
                    .with_label(*span, "missing test")
                    .with_note("every function requires at least one test")
            }

            SemanticProblem::TestTargetNotFound {
                span,
                test_name,
                target_name,
            } => {
                let test_name = interner.lookup(*test_name);
                let target_name = interner.lookup(*target_name);
                Diagnostic::error(ErrorCode::E3001)
                    .with_message(format!(
                        "test `@{test_name}` targets unknown function `@{target_name}`"
                    ))
                    .with_label(*span, "function not found")
                    .with_note("check the function name in `tests @target_name`")
            }

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
        }
    }
}

/// Convert an [`ori_canon::PatternProblem`] into a [`Diagnostic`] via [`SemanticProblem`].
///
/// Pattern problems originate from the canonicalizer's exhaustiveness/redundancy
/// checker. This function centralizes the mapping so all consumers (check command,
/// test runner, future commands) use the same conversion.
#[cold]
pub fn pattern_problem_to_diagnostic(
    problem: &ori_canon::PatternProblem,
    interner: &StringInterner,
) -> Diagnostic {
    let semantic = match problem {
        ori_canon::PatternProblem::NonExhaustive {
            match_span,
            missing,
        } => SemanticProblem::NonExhaustiveMatch {
            span: *match_span,
            missing_patterns: missing.clone(),
        },
        ori_canon::PatternProblem::RedundantArm {
            arm_span,
            match_span,
            ..
        } => SemanticProblem::RedundantPattern {
            span: *arm_span,
            covered_by_span: *match_span,
        },
    };
    semantic.into_diagnostic(interner)
}

/// Check that every function (except `@main`) has at least one test targeting it.
///
/// Returns a `SemanticProblem::MissingTest` for each untested function. This
/// centralizes test coverage analysis so all consumers (check command, test runner,
/// future `ori lint`) use the same logic.
pub fn check_test_coverage(
    module: &crate::ir::Module,
    interner: &StringInterner,
) -> Vec<SemanticProblem> {
    let main_name = interner.intern("main");

    let mut tested: rustc_hash::FxHashSet<Name> = rustc_hash::FxHashSet::default();
    for test in &module.tests {
        for target in &test.targets {
            tested.insert(*target);
        }
    }

    module
        .functions
        .iter()
        .filter(|f| f.name != main_name && !tested.contains(&f.name))
        .map(|f| SemanticProblem::MissingTest {
            span: f.span,
            func_name: f.name,
        })
        .collect()
}

#[cfg(test)]
mod tests;
