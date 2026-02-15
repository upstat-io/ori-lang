//! Attribute parsing.
//!
//! This module extends Parser with methods for parsing attributes
//! like `#skip("reason")`, `#compile_fail("error")`, `#fail("error")`,
//! `#derive(Trait1, Trait2)`, `#repr("c")`, `#target(os: "linux")`, and `#cfg(debug)`.
//!
//! Grammar: `attribute = "#" identifier [ "(" [ attribute_arg { "," attribute_arg } ] ")" ] .`
//!
//! # Extended `compile_fail` Syntax
//!
//! The `#compile_fail(...)` attribute supports rich error specifications:
//!
//! ```ori
//! // Basic format: substring match
//! #compile_fail("type mismatch")
//!
//! // Error code matching
//! #compile_fail(code: "E2001")
//!
//! // Combined message and code
//! #compile_fail(code: "E2001", message: "type mismatch")
//!
//! // Position-specific (line 1-based)
//! #compile_fail(message: "error", line: 5)
//!
//! // Full specification
//! #compile_fail(message: "error", code: "E2001", line: 5, column: 10)
//!
//! // Multiple expected errors (multiple attributes)
//! #compile_fail("type mismatch")
//! #compile_fail("unknown identifier")
//! ```

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::{CfgAttr, ExpectedError, FileAttr, Name, TargetAttr, TokenCapture, TokenKind};

/// Parsed attributes for a function or test.
///
/// Contains both the semantic attribute values and an optional token capture
/// for formatters and IDE features.
#[derive(Default, Clone, Debug)]
pub struct ParsedAttrs {
    /// Skip reason for `#skip("reason")`.
    pub skip_reason: Option<Name>,
    /// Expected compilation errors (multiple allowed).
    pub expected_errors: Vec<ExpectedError>,
    /// Expected error for `#fail("error")`.
    pub fail_expected: Option<Name>,
    /// Derived traits for `#derive(Trait1, Trait2)`.
    pub derive_traits: Vec<Name>,
    /// Repr attribute for `#repr("c")`, `#repr("packed")`, etc.
    pub repr: Option<ReprAttr>,
    /// Target conditional compilation for `#target(os: "linux")`.
    pub target: Option<TargetAttr>,
    /// Config conditional compilation for `#cfg(debug)`.
    pub cfg: Option<CfgAttr>,

    /// Token range covering all attributes (for formatters/IDE).
    ///
    /// This captures the indices of tokens from the first `#` to the last
    /// attribute closing token. Use `TokenList::get_range()` to access
    /// the actual tokens.
    pub token_range: TokenCapture,
}

/// Representation attribute values.
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "variants used when codegen consumes repr attributes"
)]
pub enum ReprAttr {
    /// `#repr("c")` - C-compatible layout
    C,
    /// `#repr("packed")` - No padding between fields
    Packed,
    /// `#repr("transparent")` - Same representation as single field
    Transparent,
    /// `#repr("aligned", N)` - Minimum alignment (power of two)
    Aligned(u64),
}

// TargetAttr and CfgAttr are defined in ori_ir and imported above.

impl ParsedAttrs {
    /// Returns true if no attributes are set.
    ///
    /// Note: This checks semantic content, not token capture.
    /// An empty `ParsedAttrs` may still have `token_range` set if there
    /// were malformed attributes that didn't parse correctly.
    pub fn is_empty(&self) -> bool {
        self.skip_reason.is_none()
            && self.expected_errors.is_empty()
            && self.fail_expected.is_none()
            && self.derive_traits.is_empty()
            && self.repr.is_none()
            && self.target.is_none()
            && self.cfg.is_none()
    }

    /// Returns true if any tokens were captured for attributes.
    #[allow(dead_code, reason = "API for formatters and IDE integration")]
    pub fn has_tokens(&self) -> bool {
        !self.token_range.is_empty()
    }
}

/// Kind of attribute being parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttrKind {
    Skip,
    CompileFail,
    Fail,
    Derive,
    Repr,
    Target,
    Cfg,
    Unknown,
}

impl AttrKind {
    fn as_str(self) -> &'static str {
        match self {
            AttrKind::Skip => "skip",
            AttrKind::CompileFail => "compile_fail",
            AttrKind::Fail => "fail",
            AttrKind::Derive => "derive",
            AttrKind::Repr => "repr",
            AttrKind::Target => "target",
            AttrKind::Cfg => "cfg",
            AttrKind::Unknown => "unknown",
        }
    }
}

impl Parser<'_> {
    /// Parse zero or more attributes: `#attr("value")` or `#derive(Trait)`.
    /// Grammar: `attribute = "#" identifier [ "(" [ attribute_arg { "," attribute_arg } ] ")" ] .`
    ///
    /// Also captures the token range for all attributes (for formatters/IDE).
    pub(crate) fn parse_attributes(&mut self, errors: &mut Vec<ParseError>) -> ParsedAttrs {
        let mut attrs = ParsedAttrs::default();

        // Start capture at the first attribute token (if any)
        let capture_start = self.cursor.start_capture();

        // Accept both old `#[...]` syntax and new `#...` syntax for backwards compatibility
        while self.cursor.check(&TokenKind::Hash) || self.cursor.check(&TokenKind::HashBracket) {
            let uses_brackets = self.cursor.check(&TokenKind::HashBracket);
            self.cursor.advance(); // consume # or #[

            let attr_kind = self.parse_attr_name(errors);

            // For unknown attributes, skip to end of attribute and continue
            if attr_kind == AttrKind::Unknown {
                if uses_brackets {
                    self.skip_to_rbracket();
                } else {
                    self.skip_to_rparen_or_newline();
                }
                continue;
            }

            match attr_kind {
                AttrKind::Derive => {
                    self.parse_derive_attr(&mut attrs, errors, uses_brackets);
                }
                AttrKind::CompileFail => {
                    self.parse_compile_fail_attr(&mut attrs, errors, uses_brackets);
                }
                AttrKind::Repr => {
                    self.parse_repr_attr(&mut attrs, errors, uses_brackets);
                }
                AttrKind::Target => {
                    self.parse_target_attr(&mut attrs, errors, uses_brackets);
                }
                AttrKind::Cfg => {
                    self.parse_cfg_attr(&mut attrs, errors, uses_brackets);
                }
                _ => {
                    self.parse_string_attr(attr_kind, &mut attrs, errors, uses_brackets);
                }
            }

            self.cursor.skip_newlines();
        }

        // Complete the capture (None if no attributes were parsed)
        attrs.token_range = self.cursor.complete_capture(capture_start);

        attrs
    }

    /// Parse the attribute name and return its kind.
    fn parse_attr_name(&mut self, errors: &mut Vec<ParseError>) -> AttrKind {
        match *self.cursor.current_kind() {
            TokenKind::Ident(name) => {
                self.cursor.advance();
                match self.cursor.interner().lookup(name) {
                    "skip" => AttrKind::Skip,
                    "compile_fail" => AttrKind::CompileFail,
                    "fail" => AttrKind::Fail,
                    "derive" => AttrKind::Derive,
                    "repr" => AttrKind::Repr,
                    "target" => AttrKind::Target,
                    "cfg" => AttrKind::Cfg,
                    s => {
                        errors.push(ParseError::new(
                            ErrorCode::E1006,
                            format!("unknown attribute '{s}'"),
                            self.cursor.previous_span(),
                        ));
                        AttrKind::Unknown
                    }
                }
            }
            TokenKind::Skip => {
                self.cursor.advance();
                AttrKind::Skip
            }
            _ => {
                errors.push(ParseError::new(
                    ErrorCode::E1004,
                    format!(
                        "expected attribute name, found {}",
                        self.cursor.current_kind().display_name()
                    ),
                    self.cursor.current_span(),
                ));
                AttrKind::Unknown
            }
        }
    }

    /// Parse a string-valued attribute like `#skip("reason")`.
    fn parse_string_attr(
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
                | AttrKind::Unknown => {}
            }
        }
    }

    /// Parse a `compile_fail` attribute with extended syntax.
    ///
    /// Supports:
    /// - `#compile_fail("message")` - simple format (message substring)
    /// - `#compile_fail(message: "msg")` - named message
    /// - `#compile_fail(code: "E2001")` - error code
    /// - `#compile_fail(message: "msg", code: "E2001", line: 5)` - combined
    fn parse_compile_fail_attr(
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
            if uses_brackets {
                self.skip_to_rbracket();
            } else {
                self.skip_to_rparen_or_newline();
            }
            return;
        }
        self.cursor.advance(); // consume (

        // Check if this is the simple format (just a string) or extended format (named args)
        if let TokenKind::String(string_name) = *self.cursor.current_kind() {
            // Simple format: #[compile_fail("message")]
            self.cursor.advance();
            attrs
                .expected_errors
                .push(ExpectedError::from_message(string_name));

            // Expect )
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
            // Extended format: #[compile_fail(name: value, ...)]
            let mut expected = ExpectedError::default();

            while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
                // Parse name: value
                let param_name = if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                    self.cursor.advance();
                    name
                } else {
                    errors.push(ParseError::new(
                        ErrorCode::E1006,
                        "expected parameter name in compile_fail",
                        self.cursor.current_span(),
                    ));
                    if uses_brackets {
                        self.skip_to_rbracket();
                    } else {
                        self.skip_to_rparen_or_newline();
                    }
                    return;
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
                    return;
                }
                self.cursor.advance();

                // Parse value based on parameter name
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

                // Comma separator (optional before closing paren)
                if self.cursor.check(&TokenKind::Comma) {
                    self.cursor.advance();
                } else if !self.cursor.check(&TokenKind::RParen) {
                    break;
                }
            }

            // Store the expected error
            if !expected.is_empty() {
                attrs.expected_errors.push(expected);
            }

            // Expect )
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

    /// Parse a derive attribute like `#derive(Eq, Clone)`.
    fn parse_derive_attr(
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

    /// Parse a `repr` attribute like `#repr("c")` or `#repr("aligned", 16)`.
    fn parse_repr_attr(
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

        // Parse repr value
        if let TokenKind::String(string_name) = *self.cursor.current_kind() {
            let repr_str = self.cursor.interner().lookup(string_name);
            let repr = match repr_str {
                "c" => Some(ReprAttr::C),
                "packed" => Some(ReprAttr::Packed),
                "transparent" => Some(ReprAttr::Transparent),
                "aligned" => {
                    self.cursor.advance(); // consume "aligned"
                                           // Expect comma and alignment value
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
                        self.cursor.previous_span(),
                    ));
                    None
                }
            };

            // For non-aligned repr, advance past the string
            if !matches!(repr, Some(ReprAttr::Aligned(_)) | None) {
                self.cursor.advance();
            }

            attrs.repr = repr;
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected repr value string",
                self.cursor.current_span(),
            ));
        }

        self.finish_attr_paren(uses_brackets, errors);
    }

    /// Parse a `target` attribute body like `(os: "linux")`, returning the `TargetAttr` directly.
    ///
    /// Expects the cursor to be positioned at the `(` token.
    /// Handles the opening `(`, named arguments, closing `)`, and optional `]`.
    fn parse_target_attr_body(
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
    fn parse_target_attr(
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
    fn parse_cfg_attr_body(
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
    fn parse_cfg_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        attrs.cfg = self.parse_cfg_attr_body(errors, uses_brackets);
    }

    /// Helper to finish parsing attribute parentheses and brackets.
    fn finish_attr_paren(&mut self, uses_brackets: bool, errors: &mut Vec<ParseError>) {
        // Expect )
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        } else {
            errors.push(ParseError::new(
                ErrorCode::E1006,
                "expected ')' to close attribute",
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

    /// Parse an optional file-level attribute: `#!target(...)` or `#!cfg(...)`.
    ///
    /// Grammar: `file_attribute = "#!" identifier "(" [ attribute_arg { "," attribute_arg } ] ")" .`
    ///
    /// Returns `None` if no `#!` token is present at the current position.
    /// Captures a span from the `#!` token start through the closing `)`.
    pub(crate) fn parse_file_attribute(
        &mut self,
        errors: &mut Vec<ParseError>,
    ) -> Option<FileAttr> {
        self.cursor.skip_newlines();

        if !self.cursor.check(&TokenKind::HashBang) {
            return None;
        }
        let start_span = self.cursor.current_span();
        self.cursor.advance(); // consume #!

        // Parse attribute name identifier
        let attr_kind = self.parse_attr_name(errors);

        match attr_kind {
            AttrKind::Target => {
                let attr = self.parse_target_attr_body(errors, false)?;
                let span = start_span.merge(self.cursor.previous_span());
                Some(FileAttr::Target { attr, span })
            }
            AttrKind::Cfg => {
                let attr = self.parse_cfg_attr_body(errors, false)?;
                let span = start_span.merge(self.cursor.previous_span());
                Some(FileAttr::Cfg { attr, span })
            }
            AttrKind::Unknown => {
                // Error already reported by parse_attr_name
                self.skip_to_rparen_or_newline();
                None
            }
            other => {
                errors.push(ParseError::new(
                    ErrorCode::E1006,
                    format!(
                        "'{}' is not valid as a file-level attribute; \
                         only 'target' and 'cfg' are allowed",
                        other.as_str()
                    ),
                    self.cursor.previous_span(),
                ));
                self.skip_to_rparen_or_newline();
                None
            }
        }
    }

    /// Skip tokens until we find a `]`.
    fn skip_to_rbracket(&mut self) {
        while !self.cursor.check(&TokenKind::RBracket) && !self.cursor.is_at_end() {
            self.cursor.advance();
        }
        if self.cursor.check(&TokenKind::RBracket) {
            self.cursor.advance();
        }
    }

    /// Skip tokens until we find a `)` or newline (for bracket-less attributes).
    fn skip_to_rparen_or_newline(&mut self) {
        while !self.cursor.check(&TokenKind::RParen)
            && !self.cursor.check(&TokenKind::Newline)
            && !self.cursor.is_at_end()
        {
            self.cursor.advance();
        }
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        }
    }
}

#[cfg(test)]
mod tests;
