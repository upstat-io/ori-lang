//! Impl block parsing.

use sigil_ir::{GenericParamRange, ImplAssocType, ImplDef, ImplMethod, TokenKind};
use crate::{ParseError, Parser};

impl Parser<'_> {
    /// Parse an impl block.
    /// Syntax: impl [<T>] Type { methods } or impl [<T>] Trait for Type { methods }
    pub(crate) fn parse_impl(&mut self) -> Result<ImplDef, ParseError> {
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
        let (trait_path, self_path, self_ty) = if self.check(&TokenKind::For) {
            self.advance();
            // Parse the implementing type
            let (impl_path, impl_ty) = self.parse_impl_type()?;
            (Some(first_path), impl_path, impl_ty)
        } else {
            (None, first_path, first_ty)
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
                    sigil_diagnostic::ErrorCode::E1001,
                    format!(
                        "expected method definition (@name) or associated type definition (type Name = ...), found {:?}",
                        self.current().kind
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
        let body = self.parse_expr()?;

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
}
