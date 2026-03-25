//! `#compile_fail(...)` attribute parsing.
//!
//! Handles both the simple format `#compile_fail("message")` and the
//! extended named-parameter format `#compile_fail(code: "E2001", message: "msg", line: 5)`.

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::{ExpectedError, Name, TokenKind};

use super::ParsedAttrs;

impl Parser<'_> {
    /// Parse a `compile_fail` attribute with extended syntax.
    ///
    /// Supports:
    /// - `#compile_fail("message")` - simple format (message substring)
    /// - `#compile_fail(message: "msg")` - named message
    /// - `#compile_fail(code: "E2001")` - error code
    /// - `#compile_fail(message: "msg", code: "E2001", line: 5)` - combined
    pub(super) fn parse_compile_fail_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected '(' after 'compile_fail'",
                self.cursor.current_span(),
            ));
            self.skip_to_attr_end(uses_brackets);
            return;
        }
        self.cursor.advance(); // consume (

        if let TokenKind::String(string_name) = *self.cursor.current_kind() {
            // Simple format: #compile_fail("message")
            self.cursor.advance();
            attrs
                .expected_errors
                .push(ExpectedError::from_message(string_name));

            if self.cursor.check(&TokenKind::RParen) {
                self.cursor.advance();
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected ')' after compile_fail value",
                    self.cursor.current_span(),
                ));
            }
        } else {
            // Extended format: #compile_fail(name: value, ...)
            self.parse_compile_fail_extended(attrs, errors, uses_brackets);
        }

        self.finish_attr_bracket(uses_brackets, errors);
    }

    /// Parse the extended `compile_fail` format with named parameters.
    ///
    /// Expects the cursor to be positioned after the opening `(`.
    /// Handles the parameter while-loop, closing `)`, and error recovery.
    fn parse_compile_fail_extended(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        let mut expected = ExpectedError::default();

        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            let param_name = if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                self.cursor.advance();
                name
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected parameter name in compile_fail",
                    self.cursor.current_span(),
                ));
                self.skip_to_attr_end(uses_brackets);
                return;
            };

            if !self.cursor.check(&TokenKind::Colon) {
                let name_str = self.cursor.interner().lookup(param_name);
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    format!("expected ':' after '{name_str}'"),
                    self.cursor.current_span(),
                ));
                self.skip_to_attr_end(uses_brackets);
                return;
            }
            self.cursor.advance();

            self.parse_compile_fail_param(param_name, &mut expected, errors);

            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RParen) {
                break;
            }
        }

        if !expected.is_empty() {
            attrs.expected_errors.push(expected);
        }

        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected ')' after compile_fail parameters",
                self.cursor.current_span(),
            ));
        }
    }

    /// Parse a single named parameter value in the extended `compile_fail` format.
    fn parse_compile_fail_param(
        &mut self,
        param_name: Name,
        expected: &mut ExpectedError,
        errors: &mut Vec<ParseError>,
    ) {
        let param_str = self.cursor.interner().lookup(param_name);
        match param_str {
            "message" | "msg" => {
                if let TokenKind::String(s) = *self.cursor.current_kind() {
                    expected.message = Some(s);
                    self.cursor.advance();
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected string for 'message'",
                        self.cursor.current_span(),
                    ));
                }
            }
            "code" => {
                if let TokenKind::String(s) = *self.cursor.current_kind() {
                    expected.code = Some(s);
                    self.cursor.advance();
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected string for 'code'",
                        self.cursor.current_span(),
                    ));
                }
            }
            "line" => {
                if let TokenKind::Int(n) = *self.cursor.current_kind() {
                    expected.line = u32::try_from(n).ok();
                    self.cursor.advance();
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected integer for 'line'",
                        self.cursor.current_span(),
                    ));
                }
            }
            "column" | "col" => {
                if let TokenKind::Int(n) = *self.cursor.current_kind() {
                    expected.column = u32::try_from(n).ok();
                    self.cursor.advance();
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected integer for 'column'",
                        self.cursor.current_span(),
                    ));
                }
            }
            _ => {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    format!("unknown compile_fail parameter '{param_str}'"),
                    self.cursor.previous_span(),
                ));
            }
        }
    }
}
