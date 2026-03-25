//! `#target(...)` and `#cfg(...)` attribute parsing.
//!
//! Handles conditional compilation attributes for both item-level
//! (`#target`, `#cfg`) and file-level (`#!target`, `#!cfg`) forms.
//! Spec: Clause 25 — Conditional Compilation.

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::{CfgAttr, Name, TargetAttr, TokenKind};

use super::ParsedAttrs;

/// Check whether a string is a valid Ori identifier (feature name).
///
/// Spec §25.3.2: Feature names shall start with a letter or underscore,
/// contain only letters, digits, and underscores, and be non-empty.
fn is_valid_feature_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}

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

            // Parse value — single string or list of strings
            // Spec §25.2: os, arch, family, not_os, not_arch, not_family take a single string
            // Spec §25.2.5: any_os, any_arch take a list ["str1", "str2"]
            let param_str = self.cursor.interner().lookup(param_name);
            match param_str {
                "os" => self.parse_attr_string_value(&mut target.os, errors),
                "arch" => self.parse_attr_string_value(&mut target.arch, errors),
                "family" => self.parse_attr_string_value(&mut target.family, errors),
                "not_os" => self.parse_attr_string_value(&mut target.not_os, errors),
                "not_arch" => self.parse_attr_string_value(&mut target.not_arch, errors),
                "not_family" => self.parse_attr_string_value(&mut target.not_family, errors),
                "any_os" => self.parse_attr_string_list(&mut target.any_os, errors),
                "any_arch" => self.parse_attr_string_list(&mut target.any_arch, errors),
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
                        "feature" => self.parse_feature_name_value(&mut cfg.feature, errors),
                        "not_feature" => {
                            self.parse_feature_name_value(&mut cfg.not_feature, errors);
                        }
                        "any_feature" => {
                            self.parse_feature_name_list(&mut cfg.any_feature, errors);
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

    /// Parse a single string value into an `Option<Name>`.
    ///
    /// Used for target/cfg parameters that take a single string: `os: "linux"`.
    fn parse_attr_string_value(&mut self, dest: &mut Option<Name>, errors: &mut Vec<ParseError>) {
        if let TokenKind::String(s) = *self.cursor.current_kind() {
            *dest = Some(s);
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected string value",
                self.cursor.current_span(),
            ));
        }
    }

    /// Parse a single feature name string, validating it as a valid Ori identifier.
    ///
    /// Spec §25.3.2: Feature names shall be valid Ori identifiers (start with
    /// letter or underscore, contain only letters, digits, and underscores).
    fn parse_feature_name_value(&mut self, dest: &mut Option<Name>, errors: &mut Vec<ParseError>) {
        if let TokenKind::String(s) = *self.cursor.current_kind() {
            let name_str = self.cursor.interner().lookup(s);
            if is_valid_feature_name(name_str) {
                *dest = Some(s);
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E0932,
                    format!(
                        "invalid feature name `{name_str}` — feature names must be \
                         valid identifiers (start with letter or underscore, contain \
                         only letters, digits, and underscores)"
                    ),
                    self.cursor.current_span(),
                ));
            }
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected string value",
                self.cursor.current_span(),
            ));
        }
    }

    /// Parse a list of feature name strings, validating each as a valid Ori identifier.
    ///
    /// Spec §25.3.2: Feature names shall be valid Ori identifiers.
    fn parse_feature_name_list(&mut self, dest: &mut Vec<Name>, errors: &mut Vec<ParseError>) {
        // Expect [
        if !self.cursor.check(&TokenKind::LBracket) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected '[' for list value",
                self.cursor.current_span(),
            ));
            return;
        }
        self.cursor.advance();

        // Parse comma-separated feature name strings
        while !self.cursor.check(&TokenKind::RBracket) && !self.cursor.is_at_end() {
            if let TokenKind::String(s) = *self.cursor.current_kind() {
                let name_str = self.cursor.interner().lookup(s);
                if is_valid_feature_name(name_str) {
                    dest.push(s);
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E0932,
                        format!(
                            "invalid feature name `{name_str}` — feature names must be \
                             valid identifiers (start with letter or underscore, contain \
                             only letters, digits, and underscores)"
                        ),
                        self.cursor.current_span(),
                    ));
                }
                self.cursor.advance();
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected string in list",
                    self.cursor.current_span(),
                ));
                break;
            }

            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RBracket) {
                break;
            }
        }

        // Expect ]
        if self.cursor.check(&TokenKind::RBracket) {
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected ']' to close list",
                self.cursor.current_span(),
            ));
        }
    }

    /// Parse a list of strings like `["linux", "macos"]` into a `Vec<Name>`.
    ///
    /// Used for `any_os`, `any_arch`, `any_feature` — Spec §25.2.5, §25.3.3.
    fn parse_attr_string_list(&mut self, dest: &mut Vec<Name>, errors: &mut Vec<ParseError>) {
        // Expect [
        if !self.cursor.check(&TokenKind::LBracket) {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected '[' for list value",
                self.cursor.current_span(),
            ));
            return;
        }
        self.cursor.advance();

        // Parse comma-separated strings
        while !self.cursor.check(&TokenKind::RBracket) && !self.cursor.is_at_end() {
            if let TokenKind::String(s) = *self.cursor.current_kind() {
                dest.push(s);
                self.cursor.advance();
            } else {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    "expected string in list",
                    self.cursor.current_span(),
                ));
                break;
            }

            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RBracket) {
                break;
            }
        }

        // Expect ]
        if self.cursor.check(&TokenKind::RBracket) {
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected ']' to close list",
                self.cursor.current_span(),
            ));
        }
    }
}
