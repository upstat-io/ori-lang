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
        if !self.cursor.check(&TokenKind::Impl) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Impl,
                self.cursor.current_span().start as usize,
            );
        }

        self.in_error_context(crate::ErrorContext::ImplBlock, Self::parse_impl_body)
    }

    fn parse_impl_body(&mut self) -> ParseOutcome<ImplDef> {
        let start_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Impl));

        // Optional generics: <T, U: Bound>
        let generics = if self.cursor.check(&TokenKind::Lt) {
            committed!(self.parse_generics().into_result())
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the first type (could be trait or self_ty)
        // Supports both simple `Box` and generic `Box<T>`
        let (first_path, first_ty) = require!(self, self.parse_impl_type(), "type after `impl`");

        // Check for `for` keyword to determine if this is a trait impl
        let (trait_path, trait_type_args, self_path, self_ty) =
            if self.cursor.check(&TokenKind::For) {
                self.cursor.advance();
                // Parse the implementing type
                let (impl_path, impl_ty) =
                    require!(self, self.parse_impl_type(), "type after `for`");
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
        let where_clauses = if self.cursor.check(&TokenKind::Where) {
            committed!(self.parse_where_clauses().into_result())
        } else {
            Vec::new()
        };

        // Impl body: { methods and associated types }
        committed!(self.cursor.expect(&TokenKind::LBrace));
        self.cursor.skip_newlines();

        let mut methods = Vec::new();
        let mut assoc_types = Vec::new();

        while !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            if self.cursor.check(&TokenKind::Type) {
                // Associated type definition: type Item = T
                let at = committed!(self.parse_impl_assoc_type());
                assoc_types.push(at);
            } else if self.cursor.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body
                let method = committed!(self.parse_impl_method());
                methods.push(method);
            } else {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1001,
                        format!(
                            "expected method definition (@name) or associated type definition (type Name = ...), found {}",
                            self.cursor.current_kind().display_name()
                        ),
                        self.cursor.current_span(),
                    ),
                    self.cursor.current_span(),
                );
            }
            self.cursor.skip_newlines();
        }

        let end_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::RBrace));

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
        let start_span = self.cursor.current_span();

        // @name
        self.cursor.expect(&TokenKind::At)?;
        let name = self.cursor.expect_ident()?;

        // (params)
        self.cursor.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.cursor.expect(&TokenKind::RParen)?;

        // -> Type
        self.cursor.expect(&TokenKind::Arrow)?;
        let return_ty = self.parse_type_required().into_result()?;

        // = body
        self.cursor.expect(&TokenKind::Eq)?;
        self.cursor.skip_newlines();
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
        let start_span = self.cursor.current_span();

        // type
        self.cursor.expect(&TokenKind::Type)?;

        // Name
        let name = self.cursor.expect_ident()?;

        // = Type
        self.cursor.expect(&TokenKind::Eq)?;
        let ty = self.parse_type_required().into_result()?;

        let end_span = self.cursor.current_span();

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
        if !self.cursor.check(&TokenKind::Def) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Def,
                self.cursor.current_span().start as usize,
            );
        }

        self.parse_def_impl_body(visibility)
    }

    fn parse_def_impl_body(&mut self, visibility: Visibility) -> ParseOutcome<DefImplDef> {
        let start_span = self.cursor.current_span();

        // def
        committed!(self.cursor.expect(&TokenKind::Def));

        // impl
        committed!(self.cursor.expect(&TokenKind::Impl));

        // TraitName (simple identifier, no path for now)
        let trait_name = committed!(self.cursor.expect_ident());

        // Body: { methods }
        committed!(self.cursor.expect(&TokenKind::LBrace));
        self.cursor.skip_newlines();

        let mut methods = Vec::new();

        while !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            if self.cursor.check(&TokenKind::At) {
                // Method: @name (...) -> Type = body
                let method = committed!(self.parse_impl_method());
                methods.push(method);
            } else {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1001,
                        format!(
                            "expected method definition (@name) in def impl block, found {}",
                            self.cursor.current_kind().display_name()
                        ),
                        self.cursor.current_span(),
                    ),
                    self.cursor.current_span(),
                );
            }
            self.cursor.skip_newlines();
        }

        let end_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(DefImplDef {
            trait_name,
            methods,
            span: start_span.merge(end_span),
            visibility,
        })
    }
}

#[cfg(test)]
mod tests;
