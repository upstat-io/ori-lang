//! Utility methods for the type checker.

use super::types::TypeCheckError;
use super::TypeChecker;
use ori_ir::{ExprId, Function, Span};
use ori_types::{Type, TypeError};

impl TypeChecker<'_> {
    /// Validate that capabilities in a function's `uses` clause refer to valid traits.
    ///
    /// For each capability in the `uses` clause, checks that a trait with that name exists
    /// in the trait registry. If not, reports an error.
    pub(crate) fn validate_capabilities(&mut self, func: &Function) {
        for cap_ref in &func.capabilities {
            if !self.registries.traits.has_trait(cap_ref.name) {
                let cap_name = self.context.interner.lookup(cap_ref.name);
                self.push_error(
                    format!("unknown capability `{cap_name}`: capabilities must be defined traits"),
                    cap_ref.span,
                    ori_diagnostic::ErrorCode::E2012,
                );
            }
        }
    }

    /// Resolve a type, returning it unchanged.
    ///
    /// Note: Ori uses newtypes (nominally distinct types), not transparent type aliases.
    /// This function exists for API compatibility but does not resolve through newtypes
    /// since they maintain their own type identity.
    #[expect(
        clippy::unused_self,
        reason = "Maintains API compatibility; self may be needed for future resolution logic"
    )]
    pub(crate) fn resolve_through_aliases(&self, ty: &Type) -> Type {
        // Newtypes are nominally distinct - they don't resolve through to their underlying type
        ty.clone()
    }

    /// Report a type error.
    pub(crate) fn report_type_error(&mut self, err: &TypeError, span: Span) {
        let diag = err.to_diagnostic(span, self.context.interner);
        let error = TypeCheckError {
            message: diag.message.clone(),
            span,
            code: diag.code,
        };

        // If we have a diagnostic queue, use it for deduplication/limits
        if let (Some(ref mut queue), Some(ref source)) =
            (&mut self.diagnostics.queue, &self.diagnostics.source)
        {
            let is_soft = error.is_soft();
            // Add to queue - it will handle deduplication and limits
            if queue.add_with_source(diag, source, is_soft) {
                self.diagnostics.errors.push(error);
            }
        } else {
            // No queue - add directly
            self.diagnostics.errors.push(error);
        }
    }

    /// Check if the error limit has been reached.
    ///
    /// When source is provided, the diagnostic queue tracks error limits.
    /// Returns false if no source/queue is configured.
    pub fn limit_reached(&self) -> bool {
        self.diagnostics
            .queue
            .as_ref()
            .is_some_and(ori_diagnostic::queue::DiagnosticQueue::limit_reached)
    }

    /// Store the type for an expression.
    ///
    /// Converts the Type to `TypeId` for efficient storage.
    pub(crate) fn store_type(&mut self, expr_id: ExprId, ty: &Type) {
        let type_id = ty.to_type_id(self.inference.ctx.interner());
        self.inference.expr_types.insert(expr_id.index(), type_id);
    }

    /// Push a type check error with the given message, span, and error code.
    ///
    /// This is a convenience method for the common pattern of creating
    /// and pushing a `TypeCheckError` to the diagnostics list.
    #[inline]
    pub(crate) fn push_error(
        &mut self,
        msg: impl Into<String>,
        span: Span,
        code: ori_diagnostic::ErrorCode,
    ) {
        self.diagnostics.errors.push(TypeCheckError {
            message: msg.into(),
            span,
            code,
        });
    }
}
