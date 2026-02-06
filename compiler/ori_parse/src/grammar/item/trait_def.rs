//! Trait definition parsing.

use crate::{ParseError, ParseOutcome, Parser};
use ori_ir::{
    GenericParamRange, TokenKind, TraitAssocType, TraitDef, TraitDefaultMethod, TraitItem,
    TraitMethodSig, Visibility,
};

impl Parser<'_> {
    /// Parse a trait definition with outcome tracking.
    pub(crate) fn parse_trait_with_outcome(
        &mut self,
        visibility: Visibility,
    ) -> ParseOutcome<TraitDef> {
        self.with_outcome(|p| p.parse_trait(visibility))
    }

    /// Parse a trait definition.
    /// Syntax: [pub] trait Name [<T>] [: Super] { items }
    pub(crate) fn parse_trait(&mut self, visibility: Visibility) -> Result<TraitDef, ParseError> {
        self.in_error_context_result(crate::ErrorContext::TraitDef, |p| {
            p.parse_trait_inner(visibility)
        })
    }

    fn parse_trait_inner(&mut self, visibility: Visibility) -> Result<TraitDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Trait)?;

        // Trait name
        let name = self.expect_ident()?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Optional super-traits: : Parent + OtherTrait
        let super_traits = if self.check(&TokenKind::Colon) {
            self.advance();
            self.parse_bounds()?
        } else {
            Vec::new()
        };

        // Trait body: { items }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_trait_item() {
                Ok(item) => items.push(item),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(TraitDef {
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
                Some(self.parse_type_required()?)
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
            let return_ty = self.parse_type_required()?;

            // Check for default implementation: = body
            if self.check(&TokenKind::Eq) {
                self.advance();
                self.skip_newlines();
                let body = self.parse_expr()?;
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
