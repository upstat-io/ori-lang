//! `#repr(...)` attribute parsing.
//!
//! Extracted from `attr/mod.rs` to keep the main module under the
//! 500-line production file limit (TPR-01-043).

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::TokenKind;

use super::{ParsedAttrs, ReprAttr};

impl Parser<'_> {
    /// Parse a single repr value string, advancing past it.
    ///
    /// Handles `"c"`, `"packed"`, `"transparent"`, and `"aligned", N` (consuming the
    /// comma and integer for aligned). Returns `None` on error (already pushed).
    pub(crate) fn parse_repr_value(&mut self, errors: &mut Vec<ParseError>) -> Option<ReprAttr> {
        let TokenKind::String(string_name) = *self.cursor.current_kind() else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected repr value string",
                self.cursor.current_span(),
            ));
            return None;
        };
        let repr_str = self.cursor.interner().lookup(string_name);
        match repr_str {
            "c" => {
                self.cursor.advance();
                Some(ReprAttr::C)
            }
            "packed" => {
                self.cursor.advance();
                Some(ReprAttr::Packed)
            }
            "transparent" => {
                self.cursor.advance();
                Some(ReprAttr::Transparent)
            }
            "aligned" => {
                self.cursor.advance(); // consume "aligned"
                if self.cursor.check(&TokenKind::Comma) {
                    self.cursor.advance();
                    if let TokenKind::Int(n) = *self.cursor.current_kind() {
                        self.cursor.advance();
                        Some(ReprAttr::Aligned(n))
                    } else {
                        errors.push(ParseError::new(
                            ErrorCode::E1006,
                            "expected alignment value after 'aligned'",
                            self.cursor.current_span(),
                        ));
                        None
                    }
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected ',' after 'aligned'",
                        self.cursor.current_span(),
                    ));
                    None
                }
            }
            s => {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    format!("unknown repr value '{s}'"),
                    self.cursor.current_span(),
                ));
                self.cursor.advance();
                None
            }
        }
    }

    /// Parse a `repr` attribute like `#repr("c")` or `#repr("aligned", 16)`.
    pub(crate) fn parse_repr_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected '(' after 'repr'",
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

        // Parse first repr value.
        if let Some(r) = self.parse_repr_value(errors) {
            attrs.repr_attrs.push(r);
        }

        // Support combined syntax: #repr("c", "aligned", 16)
        if self.cursor.check(&TokenKind::Comma) {
            self.cursor.advance();
            if let Some(r) = self.parse_repr_value(errors) {
                attrs.repr_attrs.push(r);
            }
        }

        self.finish_attr_paren(uses_brackets, errors);
    }
}
