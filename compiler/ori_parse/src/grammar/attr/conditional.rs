//! `#target(...)` and `#cfg(...)` attribute parsing.
//!
//! Handles conditional compilation attributes for both item-level
//! (`#target`, `#cfg`) and file-level (`#!target`, `#!cfg`) forms.

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::{CfgAttr, TargetAttr, TokenKind};

use super::ParsedAttrs;

impl Parser<'_> {
    /// Parse a `target` attribute body like `(os: "linux")`, returning the `TargetAttr` directly.
    ///
    /// Expects the cursor to be positioned at the `(` token.
    /// Handles the opening `(`, named arguments, closing `)`, and optional `]`.
    pub(super) fn parse_target_attr_body(
        &mut self,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) -> Option<TargetAttr> {
        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected '(' after 'target'",
                self.cursor.current_span(),
            ));
            if uses_brackets {
                self.skip_to_rbracket();
            } else {
                self.skip_to_rparen_or_newline();
            }
            return None;
        }
        self.cursor.advance(); // consume (

        let mut target = TargetAttr::default();

        // Parse named arguments
        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            let param_name = if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                self.cursor.advance();
                name
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected parameter name in target",
                    self.cursor.current_span(),
                ));
                if uses_brackets {
                    self.skip_to_rbracket();
                } else {
                    self.skip_to_rparen_or_newline();
                }
                return None;
            };

            // Expect :
            if !self.cursor.check(&TokenKind::Colon) {
                let name_str = self.cursor.interner().lookup(param_name);
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    format!("expected ':' after '{name_str}'"),
                    self.cursor.current_span(),
                ));
                if uses_brackets {
                    self.skip_to_rbracket();
                } else {
                    self.skip_to_rparen_or_newline();
                }
                return None;
            }
            self.cursor.advance();

            // Parse value
            let param_str = self.cursor.interner().lookup(param_name);
            match param_str {
                "os" => {
                    if let TokenKind::String(s) = *self.cursor.current_kind() {
                        target.os = Some(s);
                        self.cursor.advance();
                    }
                }
                "arch" => {
                    if let TokenKind::String(s) = *self.cursor.current_kind() {
                        target.arch = Some(s);
                        self.cursor.advance();
                    }
                }
                "family" => {
                    if let TokenKind::String(s) = *self.cursor.current_kind() {
                        target.family = Some(s);
                        self.cursor.advance();
                    }
                }
                "not_os" => {
                    if let TokenKind::String(s) = *self.cursor.current_kind() {
                        target.not_os = Some(s);
                        self.cursor.advance();
                    }
                }
                _ => {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        format!("unknown target parameter '{param_str}'"),
                        self.cursor.previous_span(),
                    ));
                }
            }

            // Comma separator
            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RParen) {
                break;
            }
        }

        self.finish_attr_paren(uses_brackets, errors);
        Some(target)
    }

    /// Parse a `target` attribute like `#target(os: "linux")` into `ParsedAttrs`.
    pub(super) fn parse_target_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        attrs.target = self.parse_target_attr_body(errors, uses_brackets);
    }

    /// Parse a `cfg` attribute body like `(debug)` or `(feature: "name")`, returning `CfgAttr` directly.
    ///
    /// Expects the cursor to be positioned at the `(` token.
    /// Handles the opening `(`, arguments, closing `)`, and optional `]`.
    pub(super) fn parse_cfg_attr_body(
        &mut self,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) -> Option<CfgAttr> {
        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected '(' after 'cfg'",
                self.cursor.current_span(),
            ));
            if uses_brackets {
                self.skip_to_rbracket();
            } else {
                self.skip_to_rparen_or_newline();
            }
            return None;
        }
        self.cursor.advance(); // consume (

        let mut cfg = CfgAttr::default();

        // Parse arguments - can be bare identifiers or name: value
        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                self.cursor.advance();

                if self.cursor.check(&TokenKind::Colon) {
                    // Named parameter
                    self.cursor.advance();
                    let param_str = self.cursor.interner().lookup(name);
                    match param_str {
                        "feature" => {
                            if let TokenKind::String(s) = *self.cursor.current_kind() {
                                cfg.feature = Some(s);
                                self.cursor.advance();
                            }
                        }
                        "not_feature" => {
                            if let TokenKind::String(s) = *self.cursor.current_kind() {
                                cfg.not_feature = Some(s);
                                self.cursor.advance();
                            }
                        }
                        _ => {
                            errors.push(ParseError::new(
                                ErrorCode::E1006,
                                format!("unknown cfg parameter '{param_str}'"),
                                self.cursor.previous_span(),
                            ));
                        }
                    }
                } else {
                    // Bare identifier
                    let param_str = self.cursor.interner().lookup(name);
                    match param_str {
                        "debug" => cfg.debug = true,
                        "release" => cfg.release = true,
                        "not_debug" => cfg.not_debug = true,
                        _ => {
                            errors.push(ParseError::new(
                                ErrorCode::E1006,
                                format!("unknown cfg flag '{param_str}'"),
                                self.cursor.previous_span(),
                            ));
                        }
                    }
                }
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected cfg parameter",
                    self.cursor.current_span(),
                ));
                break;
            }

            // Comma separator
            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RParen) {
                break;
            }
        }

        self.finish_attr_paren(uses_brackets, errors);
        Some(cfg)
    }

    /// Parse a `cfg` attribute like `#cfg(debug)` or `#cfg(feature: "name")` into `ParsedAttrs`.
    pub(super) fn parse_cfg_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        attrs.cfg = self.parse_cfg_attr_body(errors, uses_brackets);
    }
}
