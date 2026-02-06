//! Impl block parsing.

use crate::context::ParseContext;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    DefImplDef, GenericParamRange, ImplAssocType, ImplDef, ImplMethod, ParsedTypeRange, TokenKind,
    Visibility,
};

impl Parser<'_> {
    /// Parse an impl block.
    ///
    /// Syntax: impl [<T>] Type { methods } or impl [<T>] Trait for Type { methods }
    ///
    /// Returns `EmptyErr` if no `impl` keyword is present.
    pub(crate) fn parse_impl(&mut self) -> ParseOutcome<ImplDef> {
        if !self.check(&TokenKind::Impl) {
            return ParseOutcome::empty_err_expected(&TokenKind::Impl, self.position());
        }

        self.in_error_context(crate::ErrorContext::ImplBlock, Self::parse_impl_body)
    }

    fn parse_impl_body(&mut self) -> ParseOutcome<ImplDef> {
        let start_span = self.current_span();
        committed!(self.expect(&TokenKind::Impl));

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            committed!(self.parse_generics().into_result())
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the first type (could be trait or self_ty)
        // Supports both simple `Box` and generic `Box<T>`
        let (first_path, first_ty) = require!(self, self.parse_impl_type(), "type after `impl`");

        // Check for `for` keyword to determine if this is a trait impl
        let (trait_path, trait_type_args, self_path, self_ty) = if self.check(&TokenKind::For) {
            self.advance();
            // Parse the implementing type
            let (impl_path, impl_ty) = require!(self, self.parse_impl_type(), "type after `for`");
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
            committed!(self.parse_where_clauses().into_result())
        } else {
            Vec::new()
        };

        // Impl body: { methods and associated types }
        committed!(self.expect(&TokenKind::LBrace));
        self.skip_newlines();

        let mut methods = Vec::new();
        let mut assoc_types = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Type) {
                // Associated type definition: type Item = T
                let at = committed!(self.parse_impl_assoc_type());
                assoc_types.push(at);
            } else if self.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body
                let method = committed!(self.parse_impl_method());
                methods.push(method);
            } else {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1001,
                        format!(
                            "expected method definition (@name) or associated type definition (type Name = ...), found {}",
                            self.current_kind().display_name()
                        ),
                        self.current_span(),
                    ),
                    self.current_span(),
                );
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        committed!(self.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(ImplDef {
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
        let return_ty = self.parse_type_required().into_result()?;

        // = body
        self.expect(&TokenKind::Eq)?;
        self.skip_newlines();
        let body = self
            .with_context(ParseContext::IN_FUNCTION, Self::parse_expr)
            .into_result()?;

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
        let ty = self.parse_type_required().into_result()?;

        let end_span = self.current_span();

        Ok(ImplAssocType {
            name,
            ty,
            span: start_span.merge(end_span),
        })
    }

    /// Parse a default implementation block.
    ///
    /// Syntax: `[pub] def impl TraitName { methods }`
    ///
    /// Returns `EmptyErr` if no `def` keyword is present.
    ///
    /// Unlike regular `impl`:
    /// - No type parameters (no generics)
    /// - No `for Type` clause (anonymous implementation)
    /// - Methods must not have `self` parameter (stateless)
    pub(crate) fn parse_def_impl(&mut self, visibility: Visibility) -> ParseOutcome<DefImplDef> {
        if !self.check(&TokenKind::Def) {
            return ParseOutcome::empty_err_expected(&TokenKind::Def, self.position());
        }

        self.parse_def_impl_body(visibility)
    }

    fn parse_def_impl_body(&mut self, visibility: Visibility) -> ParseOutcome<DefImplDef> {
        let start_span = self.current_span();

        // def
        committed!(self.expect(&TokenKind::Def));

        // impl
        committed!(self.expect(&TokenKind::Impl));

        // TraitName (simple identifier, no path for now)
        let trait_name = committed!(self.expect_ident());

        // Body: { methods }
        committed!(self.expect(&TokenKind::LBrace));
        self.skip_newlines();

        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body
                let method = committed!(self.parse_impl_method());
                methods.push(method);
            } else {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1001,
                        format!(
                            "expected method definition (@name) in def impl block, found {}",
                            self.current_kind().display_name()
                        ),
                        self.current_span(),
                    ),
                    self.current_span(),
                );
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        committed!(self.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(DefImplDef {
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
