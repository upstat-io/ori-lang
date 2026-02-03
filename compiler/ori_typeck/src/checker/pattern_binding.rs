//! Pattern Binding
//!
//! Handles binding patterns to types for let expressions and function parameters.

use super::TypeChecker;
use ori_ir::{BindingPattern, Span};
use ori_types::Type;

impl TypeChecker<'_> {
    /// Bind a pattern to a type with generalization (for let-polymorphism).
    ///
    /// This is the key to Hindley-Milner let-polymorphism: we generalize
    /// the type before binding, so that `let id = x -> x` has type `∀a. a -> a`
    /// and each use of `id` gets fresh type variables.
    ///
    /// The `span` parameter is used for error reporting when pattern binding fails.
    #[allow(clippy::only_used_in_recursion)] // span is passed for future error reporting
    pub(crate) fn bind_pattern_generalized(
        &mut self,
        pattern: &BindingPattern,
        ty: Type,
        span: Span,
    ) {
        // Collect free vars in the environment to avoid generalizing over them
        let env_free_vars = self.inference.env.free_vars(&self.inference.ctx);

        match pattern {
            BindingPattern::Name(name) => {
                // Generalize the type: quantify over free vars not in environment
                let scheme = self.inference.ctx.generalize(&ty, &env_free_vars);
                self.inference.env.bind_scheme(*name, scheme);
            }
            BindingPattern::Tuple(patterns) => {
                // For tuple destructuring, each element gets generalized separately
                if let Type::Tuple(elem_types) = ty {
                    if patterns.len() == elem_types.len() {
                        for (pat, elem_ty) in patterns.iter().zip(elem_types) {
                            self.bind_pattern_generalized(pat, elem_ty, span);
                        }
                    }
                }
            }
            BindingPattern::Struct { fields } => {
                // For struct destructuring, bind each field with generalization
                for (field_name, opt_pattern) in fields {
                    let field_ty = self.inference.ctx.fresh_var();
                    if let Some(nested) = opt_pattern {
                        self.bind_pattern_generalized(nested, field_ty, span);
                    } else {
                        let scheme = self.inference.ctx.generalize(&field_ty, &env_free_vars);
                        self.inference.env.bind_scheme(*field_name, scheme);
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                // For list destructuring, each element gets generalized
                if let Type::List(elem_ty) = &ty {
                    for elem_pat in elements {
                        self.bind_pattern_generalized(elem_pat, (**elem_ty).clone(), span);
                    }
                    if let Some(rest_name) = rest {
                        let scheme = self.inference.ctx.generalize(&ty, &env_free_vars);
                        self.inference.env.bind_scheme(*rest_name, scheme);
                    }
                }
            }
            BindingPattern::Wildcard => {}
        }
    }

    /// Bind a pattern to a type (for let bindings with destructuring).
    /// This is the non-generalizing version used for function parameters.
    ///
    /// The `span` parameter is used for error reporting when pattern binding fails.
    pub(crate) fn bind_pattern(&mut self, pattern: &BindingPattern, ty: Type, span: Span) {
        match pattern {
            BindingPattern::Name(name) => {
                self.inference.env.bind(*name, ty);
            }
            BindingPattern::Tuple(patterns) => {
                // For tuple destructuring, we need to unify with a tuple type
                let resolved = self.inference.ctx.resolve(&ty);
                match resolved {
                    Type::Tuple(elem_types) => {
                        if patterns.len() == elem_types.len() {
                            for (pat, elem_ty) in patterns.iter().zip(elem_types) {
                                self.bind_pattern(pat, elem_ty, span);
                            }
                        } else {
                            self.error_tuple_length_mismatch(
                                span,
                                elem_types.len(),
                                patterns.len(),
                            );
                        }
                    }
                    Type::Var(_) => {
                        // Type variable - bind patterns to fresh vars
                        for pat in patterns {
                            let fresh_ty = self.inference.ctx.fresh_var();
                            self.bind_pattern(pat, fresh_ty, span);
                        }
                    }
                    Type::Error => {} // Error recovery - don't cascade errors
                    other => {
                        let found_type = other.display(self.context.interner);
                        self.error_tuple_pattern_mismatch(span, found_type);
                    }
                }
            }
            BindingPattern::Struct { fields } => {
                // For struct destructuring, bind each field
                for (field_name, opt_pattern) in fields {
                    let field_ty = self.inference.ctx.fresh_var();
                    match opt_pattern {
                        Some(nested) => self.bind_pattern(nested, field_ty, span),
                        None => self.inference.env.bind(*field_name, field_ty),
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                // For list destructuring, bind each element
                let resolved = self.inference.ctx.resolve(&ty);
                match resolved {
                    Type::List(elem_ty) => {
                        for elem_pat in elements {
                            self.bind_pattern(elem_pat, (*elem_ty).clone(), span);
                        }
                        if let Some(rest_name) = rest {
                            self.inference.env.bind(*rest_name, ty.clone());
                        }
                    }
                    Type::Var(_) => {
                        // Type variable - bind patterns to fresh vars
                        let elem_ty = self.inference.ctx.fresh_var();
                        for elem_pat in elements {
                            self.bind_pattern(elem_pat, elem_ty.clone(), span);
                        }
                        if let Some(rest_name) = rest {
                            self.inference
                                .env
                                .bind(*rest_name, Type::List(Box::new(elem_ty)));
                        }
                    }
                    Type::Error => {} // Error recovery - don't cascade errors
                    other => {
                        let found_type = other.display(self.context.interner);
                        self.error_list_pattern_mismatch(span, found_type);
                    }
                }
            }
            BindingPattern::Wildcard => {}
        }
    }
}
