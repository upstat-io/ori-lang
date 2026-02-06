//! Trait definition parsing.

use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    GenericParamRange, TokenKind, TraitAssocType, TraitDef, TraitDefaultMethod, TraitItem,
    TraitMethodSig, Visibility,
};

impl Parser<'_> {
    /// Parse a trait definition.
    ///
    /// Syntax: [pub] trait Name [<T>] [: Super] { items }
    ///
    /// Returns `EmptyErr` if no `trait` keyword is present.
    pub(crate) fn parse_trait(&mut self, visibility: Visibility) -> ParseOutcome<TraitDef> {
        if !self.check(&TokenKind::Trait) {
            return ParseOutcome::empty_err_expected(&TokenKind::Trait, self.position());
        }

        self.in_error_context(crate::ErrorContext::TraitDef, |p| {
            p.parse_trait_body(visibility)
        })
    }

    fn parse_trait_body(&mut self, visibility: Visibility) -> ParseOutcome<TraitDef> {
        let start_span = self.current_span();
        committed!(self.expect(&TokenKind::Trait));

        // Trait name
        let name = committed!(self.expect_ident());

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            committed!(self.parse_generics().into_result())
        } else {
            GenericParamRange::EMPTY
        };

        // Optional super-traits: : Parent + OtherTrait
        let super_traits = if self.check(&TokenKind::Colon) {
            self.advance();
            require!(self, self.parse_bounds(), "super-trait bounds after `:`")
        } else {
            Vec::new()
        };

        // Trait body: { items }
        committed!(self.expect(&TokenKind::LBrace));
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let item = committed!(self.parse_trait_item());
            items.push(item);
            self.skip_newlines();
        }

        let end_span = self.current_span();
        committed!(self.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(TraitDef {
            name,
            generics,
            super_traits,
            items,
            span: start_span.merge(end_span),
            visibility,
        })
    }

    /// Parse a single trait item (method signature, default method, or associated type).
    fn parse_trait_item(&mut self) -> Result<TraitItem, ParseError> {
        if self.check(&TokenKind::Type) {
            // Associated type: type Item or type Item = DefaultType
            let start_span = self.current_span();
            self.advance(); // consume `type`
            let name = self.expect_ident()?;

            // Optional default type: = Type
            let default_type = if self.check(&TokenKind::Eq) {
                self.advance();
                Some(self.parse_type_required().into_result()?)
            } else {
                None
            };

            Ok(TraitItem::AssocType(TraitAssocType {
                name,
                default_type,
                span: start_span.merge(self.previous_span()),
            }))
        } else if self.check(&TokenKind::At) {
            // Method: @name (params) -> Type [= body]
            let start_span = self.current_span();
            self.advance(); // consume `@`
            let name = self.expect_ident()?;

            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type
            self.expect(&TokenKind::Arrow)?;
            let return_ty = self.parse_type_required().into_result()?;

            // Check for default implementation: = body
            if self.check(&TokenKind::Eq) {
                self.advance();
                self.skip_newlines();
                let body = self.parse_expr().into_result()?;
                let end_span = self.arena.get_expr(body).span;
                Ok(TraitItem::DefaultMethod(TraitDefaultMethod {
                    name,
                    params,
                    return_ty,
                    body,
                    span: start_span.merge(end_span),
                }))
            } else {
                Ok(TraitItem::MethodSig(TraitMethodSig {
                    name,
                    params,
                    return_ty,
                    span: start_span.merge(self.previous_span()),
                }))
            }
        } else {
            Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!(
                    "expected trait item (method or associated type), found {}",
                    self.current_kind().display_name()
                ),
                self.current_span(),
            ))
        }
    }
}
