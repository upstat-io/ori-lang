//! Simple attribute parsing: `#skip`, `#fail`, and `#derive`.
//!
//! These attributes have straightforward syntax: either a single string
//! argument (`#skip("reason")`, `#fail("error")`) or a comma-separated
//! identifier list (`#derive(Eq, Clone)`).

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::TokenKind;

use super::{AttrKind, ParsedAttrs};

impl Parser<'_> {
    /// Parse a string-valued attribute like `#skip("reason")`.
    pub(super) fn parse_string_attr(
        &mut self,
        attr_kind: AttrKind,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        let attr_name_str = attr_kind.as_str();

        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                format!("expected '(' after attribute name '{attr_name_str}'"),
                self.cursor.current_span(),
            ));
            if uses_brackets {
                self.skip_to_rbracket();
            }
            return;
        }
        self.cursor.advance(); // consume (

        // Parse string value
        let value = if let TokenKind::String(string_name) = *self.cursor.current_kind() {
            self.cursor.advance();
            Some(string_name)
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                format!("attribute '{attr_name_str}' requires a string argument"),
                self.cursor.current_span(),
            ));
            None
        };

        // Expect )
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected ')' after attribute value",
                self.cursor.current_span(),
            ));
        }

        // Expect ] only if old bracket syntax was used
        if uses_brackets {
            if self.cursor.check(&TokenKind::RBracket) {
                self.cursor.advance();
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected ']' to close attribute",
                    self.cursor.current_span(),
                ));
            }
        }

        // Store the attribute
        if let Some(value) = value {
            match attr_kind {
                AttrKind::Skip => attrs.skip_reason = Some(value),
                AttrKind::Fail => attrs.fail_expected = Some(value),
                AttrKind::CompileFail
                | AttrKind::Derive
                | AttrKind::Repr
                | AttrKind::Target
                | AttrKind::Cfg
                | AttrKind::Fbip
                | AttrKind::Unknown => {}
            }
        }
    }

    /// Parse a derive attribute like `#derive(Eq, Clone)`.
    pub(super) fn parse_derive_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected '(' after 'derive'",
                self.cursor.current_span(),
            ));
            if uses_brackets {
                self.skip_to_rbracket();
            } else {
                self.skip_to_rparen_or_newline();
            }
            return;
        }
        self.cursor.advance(); // consume (

        // Parse trait list: Trait1, Trait2, ...
        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            match self.cursor.expect_ident() {
                Ok(name) => {
                    attrs.derive_traits.push(name);
                }
                Err(e) => {
                    errors.push(e);
                    if uses_brackets {
                        self.skip_to_rbracket();
                    } else {
                        self.skip_to_rparen_or_newline();
                    }
                    return;
                }
            }

            // Comma separator (optional before closing paren)
            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else {
                break;
            }
        }

        // Expect )
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected ')' after derive trait list",
                self.cursor.current_span(),
            ));
        }

        // Expect ] only if old bracket syntax was used
        if uses_brackets {
            if self.cursor.check(&TokenKind::RBracket) {
                self.cursor.advance();
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected ']' to close attribute",
                    self.cursor.current_span(),
                ));
            }
        }
    }
}
