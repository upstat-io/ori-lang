//! Impl block parsing.

use crate::context::ParseContext;
use crate::{ParseError, ParseOutcome, Parser};
use ori_ir::{
    DefImplDef, GenericParamRange, ImplAssocType, ImplDef, ImplMethod, ParsedTypeRange, TokenKind,
    Visibility,
};

impl Parser<'_> {
    /// Parse an impl block with outcome tracking.
    pub(crate) fn parse_impl_with_outcome(&mut self) -> ParseOutcome<ImplDef> {
        self.with_outcome(Self::parse_impl)
    }

    /// Parse an impl block.
    /// Syntax: impl [<T>] Type { methods } or impl [<T>] Trait for Type { methods }
    pub(crate) fn parse_impl(&mut self) -> Result<ImplDef, ParseError> {
        self.in_error_context_result(crate::ErrorContext::ImplBlock, Self::parse_impl_inner)
    }

    fn parse_impl_inner(&mut self) -> Result<ImplDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Impl)?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the first type (could be trait or self_ty)
        // Supports both simple `Box` and generic `Box<T>`
        let (first_path, first_ty) = self.parse_impl_type()?;

        // Check for `for` keyword to determine if this is a trait impl
        let (trait_path, trait_type_args, self_path, self_ty) = if self.check(&TokenKind::For) {
            self.advance();
            // Parse the implementing type
            let (impl_path, impl_ty) = self.parse_impl_type()?;
            // Extract type args from trait type (first_ty is a ParsedType::Named with type_args)
            let trait_type_args = match &first_ty {
                ori_ir::ParsedType::Named { type_args, .. } => *type_args,
                _ => ParsedTypeRange::EMPTY,
            };
            (Some(first_path), trait_type_args, impl_path, impl_ty)
        } else {
            (None, ParsedTypeRange::EMPTY, first_path, first_ty)
        };

        // Optional where clause
        let where_clauses = if self.check(&TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        // Impl body: { methods and associated types }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        let mut assoc_types = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Type) {
                // Associated type definition: type Item = T
                match self.parse_impl_assoc_type() {
                    Ok(at) => assoc_types.push(at),
                    Err(e) => return Err(e),
                }
            } else if self.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body
                match self.parse_impl_method() {
                    Ok(method) => methods.push(method),
                    Err(e) => return Err(e),
                }
            } else {
                return Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1001,
                    format!(
                        "expected method definition (@name) or associated type definition (type Name = ...), found {}",
                        self.current_kind().display_name()
                    ),
                    self.current_span(),
                ));
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(ImplDef {
            generics,
            trait_path,
            trait_type_args,
            self_path,
            self_ty,
            where_clauses,
            methods,
            assoc_types,
            span: start_span.merge(end_span),
        })
    }

    /// Parse a method in an impl block.
    pub(crate) fn parse_impl_method(&mut self) -> Result<ImplMethod, ParseError> {
        let start_span = self.current_span();

        // @name
        self.expect(&TokenKind::At)?;
        let name = self.expect_ident()?;

        // (params)
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        // -> Type
        self.expect(&TokenKind::Arrow)?;
        let return_ty = self.parse_type_required()?;

        // = body
        self.expect(&TokenKind::Eq)?;
        self.skip_newlines();
        let body = self.with_context(ParseContext::IN_FUNCTION, Self::parse_expr)?;

        let end_span = self.arena.get_expr(body).span;

        Ok(ImplMethod {
            name,
            params,
            return_ty,
            body,
            span: start_span.merge(end_span),
        })
    }

    /// Parse an associated type definition in an impl block.
    /// Syntax: type Name = Type
    fn parse_impl_assoc_type(&mut self) -> Result<ImplAssocType, ParseError> {
        let start_span = self.current_span();

        // type
        self.expect(&TokenKind::Type)?;

        // Name
        let name = self.expect_ident()?;

        // = Type
        self.expect(&TokenKind::Eq)?;
        let ty = self.parse_type_required()?;

        let end_span = self.current_span();

        Ok(ImplAssocType {
            name,
            ty,
            span: start_span.merge(end_span),
        })
    }

    /// Parse a default implementation block with outcome tracking.
    pub(crate) fn parse_def_impl_with_outcome(
        &mut self,
        visibility: Visibility,
    ) -> ParseOutcome<DefImplDef> {
        self.with_outcome(|p| p.parse_def_impl(visibility))
    }

    /// Parse a default implementation block.
    ///
    /// Syntax: `[pub] def impl TraitName { methods }`
    ///
    /// Unlike regular `impl`:
    /// - No type parameters (no generics)
    /// - No `for Type` clause (anonymous implementation)
    /// - Methods must not have `self` parameter (stateless)
    pub(crate) fn parse_def_impl(
        &mut self,
        visibility: Visibility,
    ) -> Result<DefImplDef, ParseError> {
        let start_span = self.current_span();

        // def
        self.expect(&TokenKind::Def)?;

        // impl
        self.expect(&TokenKind::Impl)?;

        // TraitName (simple identifier, no path for now)
        let trait_name = self.expect_ident()?;

        // Body: { methods }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body
                match self.parse_impl_method() {
                    Ok(method) => methods.push(method),
                    Err(e) => return Err(e),
                }
            } else {
                return Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1001,
                    format!(
                        "expected method definition (@name) in def impl block, found {}",
                        self.current_kind().display_name()
                    ),
                    self.current_span(),
                ));
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(DefImplDef {
            trait_name,
            methods,
            span: start_span.merge(end_span),
            visibility,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{parse, ParseOutput};
    use ori_ir::StringInterner;

    fn parse_source(source: &str) -> ParseOutput {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        parse(&tokens, &interner)
    }

    #[test]
    fn test_parse_def_impl_basic() {
        let source = r#"
def impl Http {
    @get (url: str) -> str = "response"
}
"#;
        let output = parse_source(source);
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.def_impls.len(), 1);

        let def_impl = &output.module.def_impls[0];
        assert_eq!(def_impl.methods.len(), 1);
        assert!(!def_impl.visibility.is_public());
    }

    #[test]
    fn test_parse_def_impl_public() {
        let source = r#"
pub def impl Http {
    @get (url: str) -> str = "response"
}
"#;
        let output = parse_source(source);
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.def_impls.len(), 1);
        assert!(output.module.def_impls[0].visibility.is_public());
    }

    #[test]
    fn test_parse_def_impl_multiple_methods() {
        let source = r#"
def impl Http {
    @get (url: str) -> str = "get"
    @post (url: str, body: str) -> str = "post"
    @delete (url: str) -> void = ()
}
"#;
        let output = parse_source(source);
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.def_impls.len(), 1);
        assert_eq!(output.module.def_impls[0].methods.len(), 3);
    }

    #[test]
    fn test_parse_def_impl_empty() {
        // Empty def impl is valid (though semantically useless)
        let source = r"
def impl Http {
}
";
        let output = parse_source(source);
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.def_impls.len(), 1);
        assert_eq!(output.module.def_impls[0].methods.len(), 0);
    }

    #[test]
    fn test_parse_multiple_def_impls() {
        let source = r#"
pub def impl Http {
    @get (url: str) -> str = "response"
}

def impl FileSystem {
    @read (path: str) -> str = "content"
}
"#;
        let output = parse_source(source);
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.def_impls.len(), 2);
    }
}
