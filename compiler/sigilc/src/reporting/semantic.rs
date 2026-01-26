//! Semantic problem rendering.
//!
//! Renders SemanticProblem variants into user-facing Diagnostic messages.

use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::problem::SemanticProblem;
use super::Render;

impl Render for SemanticProblem {
    fn render(&self) -> Diagnostic {
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
                    diag = diag.with_suggestion(format!("did you mean `{suggestion}`?"));
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
                    diag = diag.with_suggestion(format!("did you mean `@{suggestion}`?"));
                }
                diag
            }

            SemanticProblem::UnknownConfig { span, name } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("unknown config `${name}`"))
                .with_label(*span, "config not found"),

            SemanticProblem::DuplicateDefinition {
                span,
                name,
                kind,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!("duplicate {kind} definition `{name}`"))
                .with_label(*span, "duplicate definition")
                .with_secondary_label(*first_span, "first definition here"),

            SemanticProblem::PrivateAccess { span, name, kind } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("{kind} `{name}` is private"))
                .with_label(*span, "private, cannot access"),

            SemanticProblem::ImportNotFound { span, path } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("cannot find module `{path}`"))
                .with_label(*span, "module not found"),

            SemanticProblem::ImportedItemNotFound {
                span,
                item,
                module,
            } => Diagnostic::error(ErrorCode::E2003)
                .with_message(format!("cannot find `{item}` in module `{module}`"))
                .with_label(*span, "not found in module"),

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
                .with_label(*span, "used before initialization"),

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
                        "function `@{func_name}` may recurse infinitely"
                    ))
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

            SemanticProblem::UnusedFunction { span, name } => {
                Diagnostic::warning(ErrorCode::E3003)
                    .with_message(format!("unused function `@{name}`"))
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
