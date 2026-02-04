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
                self.error_unknown_capability(cap_ref.span, cap_name);
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
        let error = TypeCheckError::Generic {
            message: diag.message.clone(),
            span,
            code: diag.code,
            suggestion: None,
        };

        // If we have a diagnostic queue, use it for deduplication/limits
        if let (Some(ref mut queue), Some(ref source)) =
            (&mut self.diagnostics.queue, &self.diagnostics.source)
        {
            use ori_diagnostic::queue::DiagnosticSeverity;
            let severity = if error.is_soft() {
                DiagnosticSeverity::Soft
            } else {
                DiagnosticSeverity::Hard
            };
            // Add to queue - it will handle deduplication and limits
            if queue.add_with_source_and_severity(diag, source, severity) {
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
    ///
    /// Note: This uses the `Generic` variant for backwards compatibility.
    /// Prefer using specific typed error methods when available.
    #[inline]
    pub(crate) fn push_error(
        &mut self,
        msg: impl Into<String>,
        span: Span,
        code: ori_diagnostic::ErrorCode,
    ) {
        self.diagnostics.errors.push(TypeCheckError::Generic {
            message: msg.into(),
            span,
            code,
            suggestion: None,
        });
    }

    /// Push a typed type check error.
    ///
    /// This is the preferred method for pushing errors as it uses
    /// the structured enum variants.
    #[inline]
    pub(crate) fn push_typed_error(&mut self, error: TypeCheckError) {
        self.diagnostics.errors.push(error);
    }

    // Typed Error Factory Methods
    //
    // These methods create and push specific TypeCheckError variants,
    // providing better type safety and more consistent error messages.

    /// Report an argument count mismatch error.
    #[inline]
    pub(crate) fn error_arg_count_mismatch(
        &mut self,
        span: Span,
        expected: usize,
        found: usize,
        func_name: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::ArgCountMismatch {
            span,
            expected,
            found,
            func_name,
        });
    }

    /// Report that named arguments are required for a function call.
    #[inline]
    pub(crate) fn error_named_args_required(&mut self, span: Span, func_name: Option<String>) {
        self.push_typed_error(TypeCheckError::NamedArgsRequired { span, func_name });
    }

    /// Report that a type has no such method.
    #[inline]
    pub(crate) fn error_no_such_method(
        &mut self,
        span: Span,
        type_name: impl Into<String>,
        method_name: impl Into<String>,
        suggestion: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::NoSuchMethod {
            span,
            type_name: type_name.into(),
            method_name: method_name.into(),
            suggestion,
        });
    }

    /// Report that a capability is missing for a function call.
    #[inline]
    pub(crate) fn error_missing_capability(
        &mut self,
        span: Span,
        func_name: impl Into<String>,
        capability: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::MissingCapability {
            span,
            func_name: func_name.into(),
            capability: capability.into(),
        });
    }

    /// Report a type is not callable.
    #[inline]
    pub(crate) fn error_not_callable(&mut self, span: Span, found_type: impl Into<String>) {
        self.push_typed_error(TypeCheckError::NotCallable {
            span,
            found_type: found_type.into(),
        });
    }

    /// Report an invalid binary operation.
    #[inline]
    pub(crate) fn error_invalid_binary_op(
        &mut self,
        span: Span,
        op: impl Into<String>,
        left_type: impl Into<String>,
        right_type: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::InvalidBinaryOp {
            span,
            op: op.into(),
            left_type: left_type.into(),
            right_type: right_type.into(),
        });
    }

    /// Report an invalid unary operation.
    #[inline]
    pub(crate) fn error_invalid_unary_op(
        &mut self,
        span: Span,
        op: impl Into<String>,
        operand_type: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::InvalidUnaryOp {
            span,
            op: op.into(),
            operand_type: operand_type.into(),
        });
    }

    /// Report an operator type mismatch.
    #[inline]
    pub(crate) fn error_operator_type_mismatch(
        &mut self,
        span: Span,
        trait_name: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::OperatorTypeMismatch {
            span,
            trait_name: trait_name.into(),
            expected: expected.into(),
            found: found.into(),
        });
    }

    /// Report an unknown capability error.
    #[inline]
    pub(crate) fn error_unknown_capability(&mut self, span: Span, name: impl Into<String>) {
        self.push_typed_error(TypeCheckError::UnknownCapability {
            span,
            name: name.into(),
        });
    }

    /// Report a no such field error.
    #[inline]
    pub(crate) fn error_no_such_field(
        &mut self,
        span: Span,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        suggestion: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::NoSuchField {
            span,
            type_name: type_name.into(),
            field_name: field_name.into(),
            suggestion,
        });
    }

    /// Report an unknown struct error.
    #[inline]
    pub(crate) fn error_unknown_struct(
        &mut self,
        span: Span,
        name: impl Into<String>,
        suggestion: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::UnknownStruct {
            span,
            name: name.into(),
            suggestion,
        });
    }

    /// Report a not-a-struct error.
    #[inline]
    pub(crate) fn error_not_a_struct(&mut self, span: Span, name: impl Into<String>) {
        self.push_typed_error(TypeCheckError::NotAStruct {
            span,
            name: name.into(),
        });
    }

    /// Report a duplicate field error.
    #[inline]
    pub(crate) fn error_duplicate_field(&mut self, span: Span, field_name: impl Into<String>) {
        self.push_typed_error(TypeCheckError::DuplicateField {
            span,
            field_name: field_name.into(),
        });
    }

    /// Report a missing field error.
    #[inline]
    pub(crate) fn error_missing_field(
        &mut self,
        span: Span,
        struct_name: impl Into<String>,
        field_name: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::MissingField {
            span,
            struct_name: struct_name.into(),
            field_name: field_name.into(),
        });
    }

    /// Report that spread operator requires a list type.
    #[inline]
    pub(crate) fn error_spread_requires_list(&mut self, span: Span, found_type: &Type) {
        self.push_typed_error(TypeCheckError::SpreadRequiresList {
            span,
            found_type: format!("{found_type:?}"),
        });
    }

    /// Report that spread operator in map requires a map type.
    #[inline]
    pub(crate) fn error_spread_requires_map(&mut self, span: Span, found_type: &Type) {
        self.push_typed_error(TypeCheckError::SpreadRequiresMap {
            span,
            found_type: format!("{found_type:?}"),
        });
    }

    /// Report an invalid variant pattern error.
    #[inline]
    pub(crate) fn error_invalid_variant_pattern(
        &mut self,
        span: Span,
        variant_name: impl Into<String>,
        scrutinee_type: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::InvalidVariantPattern {
            span,
            variant_name: variant_name.into(),
            scrutinee_type: scrutinee_type.into(),
        });
    }

    /// Report a tuple length mismatch error.
    #[inline]
    pub(crate) fn error_tuple_length_mismatch(
        &mut self,
        span: Span,
        expected: usize,
        found: usize,
    ) {
        self.push_typed_error(TypeCheckError::TupleLengthMismatch {
            span,
            expected,
            found,
        });
    }

    /// Report a tuple pattern mismatch error.
    #[inline]
    pub(crate) fn error_tuple_pattern_mismatch(
        &mut self,
        span: Span,
        found_type: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::TuplePatternMismatch {
            span,
            found_type: found_type.into(),
        });
    }

    /// Report a list pattern mismatch error.
    #[inline]
    pub(crate) fn error_list_pattern_mismatch(
        &mut self,
        span: Span,
        found_type: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::ListPatternMismatch {
            span,
            found_type: found_type.into(),
        });
    }

    /// Report a coherence violation error.
    #[inline]
    pub(crate) fn error_coherence_violation(
        &mut self,
        span: Span,
        message: impl Into<String>,
        existing_span: Span,
    ) {
        self.push_typed_error(TypeCheckError::CoherenceViolation {
            span,
            message: message.into(),
            existing_span,
        });
    }

    /// Report a missing associated type error.
    #[inline]
    pub(crate) fn error_missing_assoc_type(
        &mut self,
        span: Span,
        trait_name: impl Into<String>,
        type_name: impl Into<String>,
        assoc_name: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::MissingAssocType {
            span,
            trait_name: trait_name.into(),
            type_name: type_name.into(),
            assoc_name: assoc_name.into(),
        });
    }

    /// Report a missing type argument error.
    #[inline]
    pub(crate) fn error_missing_type_arg(
        &mut self,
        span: Span,
        trait_name: impl Into<String>,
        param_name: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::MissingTypeArg {
            span,
            trait_name: trait_name.into(),
            param_name: param_name.into(),
        });
    }

    /// Report a too many type arguments error.
    #[inline]
    pub(crate) fn error_too_many_type_args(
        &mut self,
        span: Span,
        trait_name: impl Into<String>,
        expected: usize,
        found: usize,
    ) {
        self.push_typed_error(TypeCheckError::TooManyTypeArgs {
            span,
            trait_name: trait_name.into(),
            expected,
            found,
        });
    }

    /// Report a trait not found error.
    #[inline]
    pub(crate) fn error_trait_not_found(&mut self, span: Span, name: impl Into<String>) {
        self.push_typed_error(TypeCheckError::TraitNotFound {
            span,
            name: name.into(),
        });
    }

    /// Report a type parameter ordering error.
    #[inline]
    pub(crate) fn error_type_param_ordering(
        &mut self,
        span: Span,
        non_default_param: impl Into<String>,
        default_param: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::TypeParamOrdering {
            span,
            non_default_param: non_default_param.into(),
            default_param: default_param.into(),
        });
    }

    /// Report a field access not supported error.
    #[inline]
    pub(crate) fn error_field_access_not_supported(
        &mut self,
        span: Span,
        type_name: impl Into<String>,
        hint: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::FieldAccessNotSupported {
            span,
            type_name: type_name.into(),
            hint,
        });
    }

    /// Report that a type is not iterable.
    #[inline]
    pub(crate) fn error_not_iterable(&mut self, span: Span, found_type: impl Into<String>) {
        self.push_typed_error(TypeCheckError::NotIterable {
            span,
            found_type: found_type.into(),
        });
    }

    /// Report an invalid try operand error.
    #[inline]
    pub(crate) fn error_invalid_try_operand(&mut self, span: Span, found_type: impl Into<String>) {
        self.push_typed_error(TypeCheckError::InvalidTryOperand {
            span,
            found_type: found_type.into(),
        });
    }

    /// Report that await is not supported.
    #[inline]
    pub(crate) fn error_await_not_supported(&mut self, span: Span) {
        self.push_typed_error(TypeCheckError::AwaitNotSupported { span });
    }

    /// Report an undefined config variable error.
    #[inline]
    pub(crate) fn error_undefined_config(&mut self, span: Span, name: impl Into<String>) {
        self.push_typed_error(TypeCheckError::UndefinedConfig {
            span,
            name: name.into(),
        });
    }

    /// Report that self is used outside an impl block.
    #[inline]
    pub(crate) fn error_self_outside_impl(&mut self, span: Span) {
        self.push_typed_error(TypeCheckError::SelfOutsideImpl { span });
    }

    /// Report that a provider does not implement a capability.
    #[inline]
    pub(crate) fn error_capability_not_implemented(
        &mut self,
        span: Span,
        provider_type: impl Into<String>,
        capability: impl Into<String>,
    ) {
        self.push_typed_error(TypeCheckError::CapabilityNotImplemented {
            span,
            provider_type: provider_type.into(),
            capability: capability.into(),
        });
    }

    /// Report an unknown identifier error.
    #[inline]
    pub(crate) fn error_unknown_identifier(
        &mut self,
        span: Span,
        name: impl Into<String>,
        suggestion: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::UnknownIdentifier {
            span,
            name: name.into(),
            suggestion,
        });
    }

    /// Report an unknown function error.
    #[inline]
    pub(crate) fn error_unknown_function(
        &mut self,
        span: Span,
        name: impl Into<String>,
        suggestion: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::UnknownFunction {
            span,
            name: name.into(),
            suggestion,
        });
    }

    /// Report a bound not satisfied error.
    #[inline]
    pub(crate) fn error_bound_not_satisfied(
        &mut self,
        span: Span,
        type_name: impl Into<String>,
        bound_name: impl Into<String>,
        generic_name: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::BoundNotSatisfied {
            span,
            type_name: type_name.into(),
            bound_name: bound_name.into(),
            generic_name,
        });
    }

    /// Report a closure self-capture error.
    #[inline]
    pub(crate) fn error_closure_self_capture(&mut self, span: Span, name: impl Into<String>) {
        self.push_typed_error(TypeCheckError::ClosureSelfCapture {
            span,
            name: name.into(),
        });
    }

    /// Report that a module has no such export.
    #[inline]
    pub(crate) fn error_no_such_export(&mut self, span: Span, item_name: impl Into<String>) {
        self.push_typed_error(TypeCheckError::NoSuchExport {
            span,
            item_name: item_name.into(),
        });
    }

    /// Report that a type is not indexable.
    #[inline]
    pub(crate) fn error_not_indexable(
        &mut self,
        span: Span,
        found_type: impl Into<String>,
        hint: Option<String>,
    ) {
        self.push_typed_error(TypeCheckError::NotIndexable {
            span,
            found_type: found_type.into(),
            hint,
        });
    }
}
