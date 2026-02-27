//! Context stack and error management for [`InferEngine`].
//!
//! Manages the inference context stack used for tracking scope, expected types,
//! and contextual information during type inference. Also handles error/warning
//! accumulation and bidirectional type checking diagnostics.

use ori_ir::Name;

use ori_diagnostic::Suggestion;

use crate::{
    diff_types, ContextKind, ErrorContext, Expected, Idx, TypeCheckError, TypeCheckWarning,
    TypeErrorKind, TypeProblem, UnifyError,
};

use super::InferEngine;

impl InferEngine<'_> {
    // Context Management

    /// Push a context onto the stack (for nested error tracking).
    pub fn push_context(&mut self, ctx: ContextKind) {
        self.context_stack.push(ctx);
    }

    /// Pop a context from the stack.
    pub fn pop_context(&mut self) -> Option<ContextKind> {
        self.context_stack.pop()
    }

    /// Get the current context (top of stack).
    pub fn current_context(&self) -> Option<&ContextKind> {
        self.context_stack.last()
    }

    /// Execute a closure with a temporary context pushed.
    ///
    /// The context is automatically popped when the closure returns.
    pub fn with_context<T, F>(&mut self, ctx: ContextKind, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.push_context(ctx);
        let result = f(self);
        self.pop_context();
        result
    }

    // Error Management

    /// Check if any errors have been accumulated.
    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get accumulated errors.
    #[inline]
    pub fn errors(&self) -> &[TypeCheckError] {
        &self.errors
    }

    /// Take accumulated errors, leaving an empty vector.
    pub fn take_errors(&mut self) -> Vec<TypeCheckError> {
        std::mem::take(&mut self.errors)
    }

    /// Push a type check error.
    pub fn push_error(&mut self, error: TypeCheckError) {
        tracing::debug!(kind = ?error.kind, "type error recorded");
        self.errors.push(error);
    }

    /// Get the current error count (for detecting new errors after a section).
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Push a type check warning.
    pub fn push_warning(&mut self, warning: TypeCheckWarning) {
        tracing::debug!(kind = ?warning.kind, "type warning recorded");
        self.warnings.push(warning);
    }

    /// Take accumulated warnings, leaving an empty vector.
    pub fn take_warnings(&mut self) -> Vec<TypeCheckWarning> {
        std::mem::take(&mut self.warnings)
    }

    /// Rewrite `UnknownIdent` errors matching `name` (added since `errors_before`)
    /// into `ClosureSelfCapture` errors.
    ///
    /// This detects patterns like `let f = () -> f` where a closure body
    /// references its own binding name.
    pub fn rewrite_self_capture_errors(&mut self, binding_name: Name, errors_before: usize) {
        for error in &mut self.errors[errors_before..] {
            if let TypeErrorKind::UnknownIdent { name, .. } = &error.kind {
                if *name == binding_name {
                    *error = TypeCheckError::closure_self_capture(error.span);
                }
            }
        }
    }

    // Bidirectional Type Checking

    /// Check a type against an expected type.
    ///
    /// This is the "check" direction of bidirectional type checking:
    /// given an expected type, verify that the inferred type matches.
    ///
    /// On unification failure, converts the error to a rich `TypeCheckError`
    /// with context and suggestions.
    #[expect(
        clippy::result_large_err,
        reason = "TypeCheckError is intentionally large for rich error context with suggestions"
    )]
    pub fn check_type(
        &mut self,
        inferred: Idx,
        expected: &Expected,
        span: ori_ir::Span,
    ) -> Result<(), TypeCheckError> {
        match self.unify.unify(inferred, expected.ty) {
            Ok(()) => Ok(()),
            Err(ref unify_err) => {
                let error = self.make_type_error(inferred, expected, span, unify_err);
                self.errors.push(error.clone());
                Err(error)
            }
        }
    }

    /// Convert a unification error to a rich type check error.
    fn make_type_error(
        &self,
        inferred: Idx,
        expected: &Expected,
        span: ori_ir::Span,
        unify_err: &UnifyError,
    ) -> TypeCheckError {
        // Resolve both types to get their final forms
        let resolved_inferred = self.unify.resolve_readonly(inferred);
        let resolved_expected = self.unify.resolve_readonly(expected.ty);

        // Identify specific problems between the types
        let problems = diff_types(self.pool(), resolved_expected, resolved_inferred);

        // Generate suggestions based on the problems
        let suggestions = self.generate_suggestions(&problems);

        // Build context from current state
        let context = ErrorContext {
            checking: self.current_context().cloned(),
            expected_because: Some(expected.origin.clone()),
            notes: self.make_context_notes(unify_err),
        };

        TypeCheckError {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: resolved_expected,
                found: resolved_inferred,
                problems,
            },
            context,
            suggestions,
        }
    }

    /// Generate suggestions based on identified problems.
    #[expect(
        clippy::unused_self,
        reason = "Will use pool for formatting when string interning is added"
    )]
    fn generate_suggestions(&self, problems: &[TypeProblem]) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        for problem in problems {
            suggestions.extend(problem.suggestions());
        }

        // Sort by priority and deduplicate
        suggestions.sort_by_key(|s| s.priority);
        suggestions.dedup_by(|a, b| a.message == b.message);

        suggestions
    }

    /// Generate context notes from a unification error.
    #[expect(
        clippy::unused_self,
        reason = "Will use pool for name resolution when string interning is added"
    )]
    fn make_context_notes(&self, unify_err: &UnifyError) -> Vec<String> {
        let mut notes = Vec::new();

        match unify_err {
            UnifyError::InfiniteType { var_id, .. } => {
                notes.push(format!(
                    "Type variable ${var_id} would create an infinite type"
                ));
            }
            UnifyError::RigidMismatch { rigid_name, .. } => {
                // Note: rigid_name is a Name which we can't resolve to string here.
                // The error formatter will need access to a string interner.
                notes.push(format!(
                    "Type parameter (id={}) is rigid and cannot be unified with a concrete type",
                    rigid_name.raw()
                ));
            }
            UnifyError::RigidRigidMismatch { rigid1, rigid2 } => {
                notes.push(format!(
                    "Type parameters (id={}) and (id={}) are different and cannot be unified",
                    rigid1.raw(),
                    rigid2.raw()
                ));
            }
            _ => {}
        }

        notes
    }
}
