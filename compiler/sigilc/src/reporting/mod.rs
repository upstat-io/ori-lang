//! Diagnostic Rendering
//!
//! Converts structured Problem types into user-facing Diagnostic messages.
//! This separates the "what went wrong" (Problem) from "how to display it"
//! (Diagnostic).
//!
//! # Design
//!
//! The `Render` trait converts problems to diagnostics with:
//! - Error code for searchability
//! - Clear message explaining what went wrong
//! - Labeled spans showing where
//! - Notes providing context
//! - Suggestions for how to fix
//!
//! Each problem type has its own rendering logic, allowing customized
//! error messages for different situations.

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::diagnostic::queue::{DiagnosticConfig, DiagnosticQueue};
use crate::problem::{ParseProblem, Problem, SemanticProblem, TypeProblem};
use crate::typeck::TypeCheckError;

/// Trait for rendering problems into diagnostics.
pub trait Render {
    /// Render this problem into a diagnostic.
    fn render(&self) -> Diagnostic;
}

impl Render for Problem {
    fn render(&self) -> Diagnostic {
        match self {
            Problem::Parse(p) => p.render(),
            Problem::Type(p) => p.render(),
            Problem::Semantic(p) => p.render(),
        }
    }
}

impl Render for ParseProblem {
    fn render(&self) -> Diagnostic {
        match self {
            ParseProblem::UnexpectedToken {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E1001)
                .with_message(format!(
                    "unexpected token: expected {}, found `{}`",
                    expected, found
                ))
                .with_label(*span, format!("expected {}", expected)),

            ParseProblem::ExpectedExpression { span, found } => {
                Diagnostic::error(ErrorCode::E1002)
                    .with_message(format!("expected expression, found `{}`", found))
                    .with_label(*span, "expected expression here")
            }

            ParseProblem::UnclosedDelimiter {
                open_span,
                expected_close,
                found_span,
            } => {
                let opener = match expected_close {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '?',
                };
                Diagnostic::error(ErrorCode::E1003)
                    .with_message(format!("unclosed delimiter `{}`", opener))
                    .with_label(*found_span, format!("expected `{}`", expected_close))
                    .with_secondary_label(*open_span, "unclosed delimiter opened here")
            }

            ParseProblem::ExpectedIdentifier { span, found } => {
                Diagnostic::error(ErrorCode::E1004)
                    .with_message(format!("expected identifier, found `{}`", found))
                    .with_label(*span, "expected identifier")
            }

            ParseProblem::ExpectedType { span, found } => Diagnostic::error(ErrorCode::E1005)
                .with_message(format!("expected type, found `{}`", found))
                .with_label(*span, "expected type annotation"),

            ParseProblem::InvalidFunctionDef { span, reason } => {
                Diagnostic::error(ErrorCode::E1006)
                    .with_message(format!("invalid function definition: {}", reason))
                    .with_label(*span, reason.clone())
            }

            ParseProblem::MissingFunctionBody { span, name } => {
                Diagnostic::error(ErrorCode::E1007)
                    .with_message(format!("function `@{}` is missing its body", name))
                    .with_label(*span, "expected `=` followed by function body")
                    .with_suggestion("add a body: @{name} (...) -> Type = expression")
            }

            ParseProblem::InvalidPatternSyntax {
                span,
                pattern_name,
                reason,
            } => Diagnostic::error(ErrorCode::E1008)
                .with_message(format!(
                    "invalid syntax in `{}` pattern: {}",
                    pattern_name, reason
                ))
                .with_label(*span, reason.clone()),

            ParseProblem::MissingPatternArg {
                span,
                pattern_name,
                arg_name,
            } => Diagnostic::error(ErrorCode::E1009)
                .with_message(format!(
                    "missing required argument `.{}:` in `{}` pattern",
                    arg_name, pattern_name
                ))
                .with_label(*span, format!("missing `.{}:`", arg_name))
                .with_suggestion(format!(
                    "add `.{}: <value>` to the pattern arguments",
                    arg_name
                )),

            ParseProblem::UnknownPatternArg {
                span,
                pattern_name,
                arg_name,
                valid_args,
            } => {
                let valid_list = valid_args.join("`, `.");
                Diagnostic::error(ErrorCode::E1010)
                    .with_message(format!(
                        "unknown argument `.{}:` in `{}` pattern",
                        arg_name, pattern_name
                    ))
                    .with_label(*span, "unknown argument")
                    .with_note(format!("valid arguments are: `.{}`", valid_list))
            }

            ParseProblem::RequiresNamedArgs {
                span,
                func_name,
                arg_count,
            } => Diagnostic::error(ErrorCode::E1011)
                .with_message(format!(
                    "function `{}` with {} arguments requires named arguments",
                    func_name, arg_count
                ))
                .with_label(*span, "use named arguments")
                .with_suggestion("use arg: value syntax for each argument"),

            ParseProblem::InvalidFunctionSeq {
                span,
                seq_name,
                reason,
            } => Diagnostic::error(ErrorCode::E1012)
                .with_message(format!("invalid `{}` expression: {}", seq_name, reason))
                .with_label(*span, reason.clone()),

            ParseProblem::RequiresNamedProps { span, exp_name } => {
                Diagnostic::error(ErrorCode::E1013)
                    .with_message(format!(
                        "`{}` requires named properties (`name: value`)",
                        exp_name
                    ))
                    .with_label(*span, "use named properties")
                    .with_suggestion(format!(
                        "example: {}(over: items, transform: fn)",
                        exp_name
                    ))
            }

            ParseProblem::ReservedBuiltinName { span, name } => {
                Diagnostic::error(ErrorCode::E1014)
                    .with_message(format!("`{}` is a reserved built-in function name", name))
                    .with_label(*span, "cannot use this name for user-defined functions")
                    .with_note("built-in names are reserved in call position")
            }

            ParseProblem::UnterminatedString { span } => Diagnostic::error(ErrorCode::E0001)
                .with_message("unterminated string literal")
                .with_label(*span, "string not closed")
                .with_suggestion("add closing `\"`"),

            ParseProblem::InvalidCharacter { span, char } => Diagnostic::error(ErrorCode::E0002)
                .with_message(format!("invalid character `{}`", char))
                .with_label(*span, "unexpected character"),

            ParseProblem::InvalidNumber { span, reason } => Diagnostic::error(ErrorCode::E0003)
                .with_message(format!("invalid number literal: {}", reason))
                .with_label(*span, reason.clone()),

            ParseProblem::UnterminatedChar { span } => Diagnostic::error(ErrorCode::E0004)
                .with_message("unterminated character literal")
                .with_label(*span, "character literal not closed")
                .with_suggestion("add closing `'`"),

            ParseProblem::InvalidEscape { span, escape } => Diagnostic::error(ErrorCode::E0005)
                .with_message(format!("invalid escape sequence `{}`", escape))
                .with_label(*span, "unknown escape")
                .with_note("valid escapes are: \\n, \\t, \\r, \\\", \\\\, \\'"),
        }
    }
}

impl Render for TypeProblem {
    fn render(&self) -> Diagnostic {
        match self {
            TypeProblem::TypeMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "type mismatch: expected `{}`, found `{}`",
                    expected, found
                ))
                .with_label(*span, format!("expected `{}`", expected)),

            TypeProblem::ArgCountMismatch {
                span,
                expected,
                found,
            } => {
                let plural = if *expected == 1 { "" } else { "s" };
                Diagnostic::error(ErrorCode::E2004)
                    .with_message(format!(
                        "wrong number of arguments: expected {}, found {}",
                        expected, found
                    ))
                    .with_label(*span, format!("expected {} argument{}", expected, plural))
                    .with_suggestion(if *found > *expected {
                        "remove extra arguments"
                    } else {
                        "add missing arguments"
                    })
            }

            TypeProblem::TupleLengthMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "tuple length mismatch: expected {}-tuple, found {}-tuple",
                    expected, found
                ))
                .with_label(*span, format!("expected {} elements", expected)),

            TypeProblem::ListLengthMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "list destructuring: expected at least {} elements, found {}",
                    expected, found
                ))
                .with_label(*span, format!("expected at least {} elements", expected)),

            TypeProblem::InfiniteType { span } => Diagnostic::error(ErrorCode::E2008)
                .with_message("infinite type detected")
                .with_label(*span, "this would create an infinite type")
                .with_note("a type cannot contain itself"),

            TypeProblem::CannotInfer { span, context } => Diagnostic::error(ErrorCode::E2005)
                .with_message(format!("cannot infer type for {}", context))
                .with_label(*span, "type annotation needed")
                .with_suggestion("add explicit type annotation"),

            TypeProblem::UnknownType { span, name } => Diagnostic::error(ErrorCode::E2002)
                .with_message(format!("unknown type `{}`", name))
                .with_label(*span, "not found"),

            TypeProblem::NotCallable { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("`{}` is not callable", found_type))
                .with_label(*span, "cannot call this as a function"),

            TypeProblem::NotIndexable { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("`{}` cannot be indexed", found_type))
                .with_label(*span, "indexing not supported"),

            TypeProblem::NoSuchField {
                span,
                type_name,
                field_name,
                available_fields,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!(
                        "no field `{}` on type `{}`",
                        field_name, type_name
                    ))
                    .with_label(*span, "unknown field");
                if !available_fields.is_empty() {
                    diag = diag.with_note(format!(
                        "available fields: {}",
                        available_fields.join(", ")
                    ));
                }
                diag
            }

            TypeProblem::NoSuchMethod {
                span,
                type_name,
                method_name,
            } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!(
                    "no method `{}` on type `{}`",
                    method_name, type_name
                ))
                .with_label(*span, "method not found"),

            TypeProblem::InvalidBinaryOp {
                span,
                op,
                left_type,
                right_type,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "cannot apply `{}` to `{}` and `{}`",
                    op, left_type, right_type
                ))
                .with_label(*span, "invalid operation"),

            TypeProblem::InvalidUnaryOp {
                span,
                op,
                operand_type,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("cannot apply `{}` to `{}`", op, operand_type))
                .with_label(*span, "invalid operation"),

            TypeProblem::MissingNamedArg { span, arg_name } => Diagnostic::error(ErrorCode::E2004)
                .with_message(format!("missing required argument `.{}:`", arg_name))
                .with_label(*span, format!("missing `.{}:`", arg_name)),

            TypeProblem::UnknownNamedArg {
                span,
                arg_name,
                valid_args,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2004)
                    .with_message(format!("unknown argument `.{}:`", arg_name))
                    .with_label(*span, "unknown argument");
                if !valid_args.is_empty() {
                    diag =
                        diag.with_note(format!("valid arguments: .{}", valid_args.join(", .")));
                }
                diag
            }

            TypeProblem::DuplicateNamedArg {
                span,
                arg_name,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!("duplicate argument `.{}:`", arg_name))
                .with_label(*span, "duplicate")
                .with_secondary_label(*first_span, "first occurrence here"),

            TypeProblem::ReturnTypeMismatch {
                span,
                expected,
                found,
                func_name,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "return type mismatch in `{}`: expected `{}`, found `{}`",
                    func_name, expected, found
                ))
                .with_label(*span, format!("expected `{}`", expected)),

            TypeProblem::InvalidTryOperand { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "`?` operator requires Result or Option, found `{}`",
                        found_type
                    ))
                    .with_label(*span, "not Result or Option")
            }

            TypeProblem::InvalidAwait { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("`await` requires async value, found `{}`", found_type))
                .with_label(*span, "not async"),

            TypeProblem::ConditionNotBool { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "condition must be `bool`, found `{}`",
                        found_type
                    ))
                    .with_label(*span, "expected `bool`")
            }

            TypeProblem::NotIterable { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("`{}` is not iterable", found_type))
                .with_label(*span, "cannot iterate over this"),

            TypeProblem::MatchArmTypeMismatch {
                span,
                first_type,
                this_type,
                first_span,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "match arms have incompatible types: `{}` vs `{}`",
                    first_type, this_type
                ))
                .with_label(*span, format!("expected `{}`", first_type))
                .with_secondary_label(*first_span, "first arm has this type"),

            TypeProblem::PatternTypeMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "pattern type mismatch: expected `{}`, found `{}`",
                    expected, found
                ))
                .with_label(*span, format!("expected `{}`", expected)),

            TypeProblem::CyclicType { span, type_name } => Diagnostic::error(ErrorCode::E2008)
                .with_message(format!("cyclic type definition for `{}`", type_name))
                .with_label(*span, "cycle detected here"),

            TypeProblem::ClosureSelfReference { span } => Diagnostic::error(ErrorCode::E2007)
                .with_message("closure cannot capture itself")
                .with_label(*span, "self-reference not allowed")
                .with_note("closures cannot recursively reference themselves"),
        }
    }
}

impl Render for SemanticProblem {
    fn render(&self) -> Diagnostic {
        match self {
            SemanticProblem::UnknownIdentifier {
                span,
                name,
                similar,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown identifier `{}`", name))
                    .with_label(*span, "not found in this scope");
                if let Some(suggestion) = similar {
                    diag = diag.with_suggestion(format!("did you mean `{}`?", suggestion));
                }
                diag
            }

            SemanticProblem::UnknownFunction {
                span,
                name,
                similar,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown function `@{}`", name))
                    .with_label(*span, "function not found");
                if let Some(suggestion) = similar {
                    diag = diag.with_suggestion(format!("did you mean `@{}`?", suggestion));
                }
                diag
            }

            SemanticProblem::UnknownConfig { span, name } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("unknown config `${}`", name))
                .with_label(*span, "config not found"),

            SemanticProblem::DuplicateDefinition {
                span,
                name,
                kind,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!("duplicate {} definition `{}`", kind, name))
                .with_label(*span, "duplicate definition")
                .with_secondary_label(*first_span, "first definition here"),

            SemanticProblem::PrivateAccess { span, name, kind } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("{} `{}` is private", kind, name))
                .with_label(*span, "private, cannot access"),

            SemanticProblem::ImportNotFound { span, path } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("cannot find module `{}`", path))
                .with_label(*span, "module not found"),

            SemanticProblem::ImportedItemNotFound {
                span,
                item,
                module,
            } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("cannot find `{}` in module `{}`", item, module))
                .with_label(*span, "not found in module"),

            SemanticProblem::ImmutableMutation {
                span,
                name,
                binding_span,
            } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("cannot mutate immutable binding `{}`", name))
                .with_label(*span, "cannot mutate")
                .with_secondary_label(*binding_span, "defined as immutable here")
                .with_suggestion("use `let mut` for mutable bindings"),

            SemanticProblem::UseBeforeInit { span, name } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("use of possibly uninitialized `{}`", name))
                .with_label(*span, "used before initialization"),

            SemanticProblem::MissingTest { span, func_name } => Diagnostic::error(ErrorCode::E3001)
                .with_message(format!("function `@{}` has no tests", func_name))
                .with_label(*span, "missing test")
                .with_note("every function requires at least one test"),

            SemanticProblem::TestTargetNotFound {
                span,
                test_name,
                target_name,
            } => Diagnostic::error(ErrorCode::E3001)
                .with_message(format!(
                    "test `@{}` targets unknown function `@{}`",
                    test_name, target_name
                ))
                .with_label(*span, "function not found"),

            SemanticProblem::BreakOutsideLoop { span } => Diagnostic::error(ErrorCode::E3002)
                .with_message("`break` outside of loop")
                .with_label(*span, "cannot break here"),

            SemanticProblem::ContinueOutsideLoop { span } => Diagnostic::error(ErrorCode::E3002)
                .with_message("`continue` outside of loop")
                .with_label(*span, "cannot continue here"),

            SemanticProblem::ReturnOutsideFunction { span } => Diagnostic::error(ErrorCode::E3002)
                .with_message("`return` outside of function")
                .with_label(*span, "cannot return here"),

            SemanticProblem::SelfOutsideMethod { span } => Diagnostic::error(ErrorCode::E3002)
                .with_message("`self` outside of method")
                .with_label(*span, "no `self` in this context"),

            SemanticProblem::InfiniteRecursion { span, func_name } => {
                Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!(
                        "function `@{}` may recurse infinitely",
                        func_name
                    ))
                    .with_label(*span, "unconditional recursion")
                    .with_suggestion("add a base case to stop recursion")
            }

            SemanticProblem::UnusedVariable { span, name } => {
                let mut diag = Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("unused variable `{}`", name))
                    .with_label(*span, "never used");
                if !name.starts_with('_') {
                    diag = diag.with_suggestion(format!("prefix with underscore: `_{}`", name));
                }
                diag
            }

            SemanticProblem::UnusedFunction { span, name } => {
                Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("unused function `@{}`", name))
                    .with_label(*span, "never called")
            }

            SemanticProblem::UnreachableCode { span } => Diagnostic::warning(ErrorCode::E3003)
                .with_message("unreachable code")
                .with_label(*span, "this code will never execute"),

            SemanticProblem::NonExhaustiveMatch {
                span,
                missing_patterns,
            } => {
                let missing = missing_patterns.join(", ");
                Diagnostic::error(ErrorCode::E3002)
                    .with_message("non-exhaustive match")
                    .with_label(*span, "patterns not covered")
                    .with_note(format!("missing patterns: {}", missing))
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
                    .with_message(format!("missing capability `{}`", capability))
                    .with_label(*span, "capability not provided")
                    .with_suggestion(format!(
                        "add `uses {}` to function signature or provide with `with...in`",
                        capability
                    ))
            }

            SemanticProblem::DuplicateCapability {
                span,
                capability,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!("duplicate capability `{}`", capability))
                .with_label(*span, "duplicate")
                .with_secondary_label(*first_span, "first provided here"),
        }
    }
}

/// Render a collection of problems into diagnostics.
pub fn render_all(problems: &[Problem]) -> Vec<Diagnostic> {
    problems.iter().map(|p| p.render()).collect()
}

/// Process type check errors through the diagnostic queue for filtering and sorting.
///
/// This applies Go-style error handling:
/// - Error limits to prevent overwhelming output
/// - Deduplication of same-line errors
/// - Soft error suppression after hard errors
/// - Follow-on error filtering
/// - Sorting by source position
///
/// # Arguments
///
/// * `errors` - Type check errors to process
/// * `source` - Source code for computing line numbers
/// * `config` - Optional configuration (uses defaults if None)
///
/// # Returns
///
/// Filtered and sorted diagnostics, ready for display.
pub fn process_type_errors(
    errors: Vec<TypeCheckError>,
    source: &str,
    config: Option<DiagnosticConfig>,
) -> Vec<Diagnostic> {
    let config = config.unwrap_or_default();
    let mut queue = DiagnosticQueue::with_config(config);

    for error in errors {
        let diag = error.to_diagnostic();
        let soft = error.is_soft();
        queue.add_with_source(diag, source, soft);
    }

    queue.flush()
}

/// Process raw diagnostics through the queue for filtering and sorting.
///
/// Similar to `process_type_errors` but works with pre-built Diagnostic objects.
pub fn process_diagnostics(
    diagnostics: Vec<Diagnostic>,
    source: &str,
    config: Option<DiagnosticConfig>,
) -> Vec<Diagnostic> {
    let config = config.unwrap_or_default();
    let mut queue = DiagnosticQueue::with_config(config);

    for diag in diagnostics {
        // All non-TypeCheckError diagnostics are considered hard errors
        queue.add_with_source(diag, source, false);
    }

    queue.flush()
}

/// A report containing multiple diagnostics.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    pub fn new() -> Self {
        Report {
            diagnostics: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Report {
            diagnostics: Vec::with_capacity(capacity),
        }
    }

    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn add_problem(&mut self, problem: &Problem) {
        self.diagnostics.push(problem.render());
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Warning))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Span;
    use crate::problem::semantic::DefinitionKind;

    #[test]
    fn test_render_parse_problem() {
        let problem = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E1001);
        assert!(diag.message.contains("unexpected token"));
        assert!(diag.message.contains("expression"));
        assert!(diag.message.contains("}"));
        assert!(diag.is_error());
    }

    #[test]
    fn test_render_type_mismatch() {
        let problem = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E2001);
        assert!(diag.message.contains("type mismatch"));
        assert!(diag.message.contains("int"));
        assert!(diag.message.contains("str"));
    }

    #[test]
    fn test_render_unknown_identifier_with_suggestion() {
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(diag.message.contains("unknown identifier"));
        assert!(diag.suggestions.iter().any(|s| s.contains("for")));
    }

    #[test]
    fn test_render_duplicate_definition() {
        let problem = SemanticProblem::DuplicateDefinition {
            span: Span::new(100, 110),
            name: "bar".into(),
            kind: DefinitionKind::Function,
            first_span: Span::new(10, 20),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E2006);
        assert!(diag.message.contains("duplicate"));
        assert!(diag.message.contains("function"));
        assert_eq!(diag.labels.len(), 2); // primary + secondary
    }

    #[test]
    fn test_render_warning() {
        let problem = SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        };

        let diag = problem.render();

        assert!(!diag.is_error());
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn test_render_all() {
        let problems = vec![
            Problem::Parse(ParseProblem::UnexpectedToken {
                span: Span::new(0, 5),
                expected: "expression".into(),
                found: "}".into(),
            }),
            Problem::Type(TypeProblem::TypeMismatch {
                span: Span::new(10, 15),
                expected: "int".into(),
                found: "str".into(),
            }),
        ];

        let diagnostics = render_all(&problems);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, ErrorCode::E1001);
        assert_eq!(diagnostics[1].code, ErrorCode::E2001);
    }

    #[test]
    fn test_report() {
        let mut report = Report::new();

        report.add_problem(&Problem::Parse(ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        }));

        report.add_problem(&Problem::Semantic(SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        }));

        assert_eq!(report.len(), 2);
        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
    }
}
