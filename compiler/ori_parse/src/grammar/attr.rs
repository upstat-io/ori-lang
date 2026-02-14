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
use ori_diagnostic::queue::DiagnosticSeverity;
use ori_diagnostic::ErrorCode;
use ori_ir::{ExpectedError, FileAttr, Name, TokenCapture, TokenKind};

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

/// Target conditional compilation attribute.
#[derive(Clone, Debug, Default)]
#[allow(
    dead_code,
    reason = "fields used when conditional compilation is implemented"
)]
pub struct TargetAttr {
    pub os: Option<Name>,
    pub arch: Option<Name>,
    pub family: Option<Name>,
    pub any_os: Vec<Name>,
    pub not_os: Option<Name>,
}

/// Config conditional compilation attribute.
#[derive(Clone, Debug, Default)]
#[allow(
    dead_code,
    reason = "fields used when conditional compilation is implemented"
)]
pub struct CfgAttr {
    pub debug: bool,
    pub release: bool,
    pub not_debug: bool,
    pub feature: Option<Name>,
    pub any_feature: Vec<Name>,
    pub not_feature: Option<Name>,
}

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
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: format!("expected '(' after attribute name '{attr_name_str}'"),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
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
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: format!("attribute '{attr_name_str}' requires a string argument"),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
            None
        };

        // Expect )
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        } else {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected ')' after attribute value".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
        }

        // Expect ] only if old bracket syntax was used
        if uses_brackets {
            if self.cursor.check(&TokenKind::RBracket) {
                self.cursor.advance();
            } else {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected ']' to close attribute".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
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
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected '(' after 'compile_fail'".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
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
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected ')' after compile_fail value".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
            }
        } else {
            // Extended format: #[compile_fail(name: value, ...)]
            let mut expected = ExpectedError::default();

            while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
                // Parse name: value
                let param_name = if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                    let s = self.cursor.interner().lookup(name).to_owned();
                    self.cursor.advance();
                    s
                } else {
                    errors.push(ParseError {
                        code: ErrorCode::E1006,
                        message: "expected parameter name in compile_fail".to_string(),
                        span: self.cursor.current_span(),
                        context: None,
                        help: Vec::new(),
                        severity: DiagnosticSeverity::Hard,
                    });
                    if uses_brackets {
                        self.skip_to_rbracket();
                    } else {
                        self.skip_to_rparen_or_newline();
                    }
                    return;
                };

                // Expect :
                if !self.cursor.check(&TokenKind::Colon) {
                    errors.push(ParseError {
                        code: ErrorCode::E1006,
                        message: format!("expected ':' after '{param_name}'"),
                        span: self.cursor.current_span(),
                        context: None,
                        help: Vec::new(),
                        severity: DiagnosticSeverity::Hard,
                    });
                    if uses_brackets {
                        self.skip_to_rbracket();
                    } else {
                        self.skip_to_rparen_or_newline();
                    }
                    return;
                }
                self.cursor.advance();

                // Parse value based on parameter name
                match param_name.as_str() {
                    "message" | "msg" => {
                        if let TokenKind::String(s) = *self.cursor.current_kind() {
                            expected.message = Some(s);
                            self.cursor.advance();
                        } else {
                            errors.push(ParseError {
                                code: ErrorCode::E1006,
                                message: "expected string for 'message'".to_string(),
                                span: self.cursor.current_span(),
                                context: None,
                                help: Vec::new(),
                                severity: DiagnosticSeverity::Hard,
                            });
                        }
                    }
                    "code" => {
                        if let TokenKind::String(s) = *self.cursor.current_kind() {
                            expected.code = Some(s);
                            self.cursor.advance();
                        } else {
                            errors.push(ParseError {
                                code: ErrorCode::E1006,
                                message: "expected string for 'code'".to_string(),
                                span: self.cursor.current_span(),
                                context: None,
                                help: Vec::new(),
                                severity: DiagnosticSeverity::Hard,
                            });
                        }
                    }
                    "line" => {
                        if let TokenKind::Int(n) = *self.cursor.current_kind() {
                            expected.line = u32::try_from(n).ok();
                            self.cursor.advance();
                        } else {
                            errors.push(ParseError {
                                code: ErrorCode::E1006,
                                message: "expected integer for 'line'".to_string(),
                                span: self.cursor.current_span(),
                                context: None,
                                help: Vec::new(),
                                severity: DiagnosticSeverity::Hard,
                            });
                        }
                    }
                    "column" | "col" => {
                        if let TokenKind::Int(n) = *self.cursor.current_kind() {
                            expected.column = u32::try_from(n).ok();
                            self.cursor.advance();
                        } else {
                            errors.push(ParseError {
                                code: ErrorCode::E1006,
                                message: "expected integer for 'column'".to_string(),
                                span: self.cursor.current_span(),
                                context: None,
                                help: Vec::new(),
                                severity: DiagnosticSeverity::Hard,
                            });
                        }
                    }
                    _ => {
                        errors.push(ParseError {
                            code: ErrorCode::E1006,
                            message: format!("unknown compile_fail parameter '{param_name}'"),
                            span: self.cursor.previous_span(),
                            context: None,
                            help: Vec::new(),
                            severity: DiagnosticSeverity::Hard,
                        });
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
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected ')' after compile_fail parameters".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
            }
        }

        // Expect ] only if old bracket syntax was used
        if uses_brackets {
            if self.cursor.check(&TokenKind::RBracket) {
                self.cursor.advance();
            } else {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected ']' to close attribute".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
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
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected '(' after 'derive'".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
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
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected ')' after derive trait list".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
        }

        // Expect ] only if old bracket syntax was used
        if uses_brackets {
            if self.cursor.check(&TokenKind::RBracket) {
                self.cursor.advance();
            } else {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected ']' to close attribute".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
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
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected '(' after 'repr'".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
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
                            errors.push(ParseError {
                                code: ErrorCode::E1006,
                                message: "expected alignment value after 'aligned'".to_string(),
                                span: self.cursor.current_span(),
                                context: None,
                                help: Vec::new(),
                                severity: DiagnosticSeverity::Hard,
                            });
                            None
                        }
                    } else {
                        errors.push(ParseError {
                            code: ErrorCode::E1006,
                            message: "expected ',' after 'aligned'".to_string(),
                            span: self.cursor.current_span(),
                            context: None,
                            help: Vec::new(),
                            severity: DiagnosticSeverity::Hard,
                        });
                        None
                    }
                }
                s => {
                    errors.push(ParseError {
                        code: ErrorCode::E1006,
                        message: format!("unknown repr value '{s}'"),
                        span: self.cursor.previous_span(),
                        context: None,
                        help: Vec::new(),
                        severity: DiagnosticSeverity::Hard,
                    });
                    None
                }
            };

            // For non-aligned repr, advance past the string
            if !matches!(repr, Some(ReprAttr::Aligned(_)) | None) {
                self.cursor.advance();
            }

            attrs.repr = repr;
        } else {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected repr value string".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
        }

        self.finish_attr_paren(uses_brackets, errors);
    }

    /// Parse a `target` attribute like `#target(os: "linux")`.
    fn parse_target_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected '(' after 'target'".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
            if uses_brackets {
                self.skip_to_rbracket();
            } else {
                self.skip_to_rparen_or_newline();
            }
            return;
        }
        self.cursor.advance(); // consume (

        let mut target = TargetAttr::default();

        // Parse named arguments
        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            let param_name = if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                let s = self.cursor.interner().lookup(name).to_owned();
                self.cursor.advance();
                s
            } else {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected parameter name in target".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
                if uses_brackets {
                    self.skip_to_rbracket();
                } else {
                    self.skip_to_rparen_or_newline();
                }
                return;
            };

            // Expect :
            if !self.cursor.check(&TokenKind::Colon) {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: format!("expected ':' after '{param_name}'"),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
                if uses_brackets {
                    self.skip_to_rbracket();
                } else {
                    self.skip_to_rparen_or_newline();
                }
                return;
            }
            self.cursor.advance();

            // Parse value
            match param_name.as_str() {
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
                    errors.push(ParseError {
                        code: ErrorCode::E1006,
                        message: format!("unknown target parameter '{param_name}'"),
                        span: self.cursor.previous_span(),
                        context: None,
                        help: Vec::new(),
                        severity: DiagnosticSeverity::Hard,
                    });
                }
            }

            // Comma separator
            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RParen) {
                break;
            }
        }

        attrs.target = Some(target);
        self.finish_attr_paren(uses_brackets, errors);
    }

    /// Parse a `cfg` attribute like `#cfg(debug)` or `#cfg(feature: "name")`.
    fn parse_cfg_attr(
        &mut self,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
        uses_brackets: bool,
    ) {
        // Expect (
        if !self.cursor.check(&TokenKind::LParen) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected '(' after 'cfg'".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
            if uses_brackets {
                self.skip_to_rbracket();
            } else {
                self.skip_to_rparen_or_newline();
            }
            return;
        }
        self.cursor.advance(); // consume (

        let mut cfg = CfgAttr::default();

        // Parse arguments - can be bare identifiers or name: value
        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                let param_name = self.cursor.interner().lookup(name).to_owned();
                self.cursor.advance();

                if self.cursor.check(&TokenKind::Colon) {
                    // Named parameter
                    self.cursor.advance();
                    match param_name.as_str() {
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
                            errors.push(ParseError {
                                code: ErrorCode::E1006,
                                message: format!("unknown cfg parameter '{param_name}'"),
                                span: self.cursor.previous_span(),
                                context: None,
                                help: Vec::new(),
                                severity: DiagnosticSeverity::Hard,
                            });
                        }
                    }
                } else {
                    // Bare identifier
                    match param_name.as_str() {
                        "debug" => cfg.debug = true,
                        "release" => cfg.release = true,
                        "not_debug" => cfg.not_debug = true,
                        _ => {
                            errors.push(ParseError {
                                code: ErrorCode::E1006,
                                message: format!("unknown cfg flag '{param_name}'"),
                                span: self.cursor.previous_span(),
                                context: None,
                                help: Vec::new(),
                                severity: DiagnosticSeverity::Hard,
                            });
                        }
                    }
                }
            } else {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected cfg parameter".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
                break;
            }

            // Comma separator
            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else if !self.cursor.check(&TokenKind::RParen) {
                break;
            }
        }

        attrs.cfg = Some(cfg);
        self.finish_attr_paren(uses_brackets, errors);
    }

    /// Helper to finish parsing attribute parentheses and brackets.
    fn finish_attr_paren(&mut self, uses_brackets: bool, errors: &mut Vec<ParseError>) {
        // Expect )
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        } else {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected ')' to close attribute".to_string(),
                span: self.cursor.current_span(),
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            });
        }

        // Expect ] only if old bracket syntax was used
        if uses_brackets {
            if self.cursor.check(&TokenKind::RBracket) {
                self.cursor.advance();
            } else {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: "expected ']' to close attribute".to_string(),
                    span: self.cursor.current_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
            }
        }
    }

    /// Parse an optional file-level attribute: `#!target(...)` or `#!cfg(...)`.
    ///
    /// Grammar: `file_attribute = "#!" identifier "(" [ attribute_arg { "," attribute_arg } ] ")" .`
    ///
    /// Returns `None` if no `#!` token is present at the current position.
    pub(crate) fn parse_file_attribute(
        &mut self,
        errors: &mut Vec<ParseError>,
    ) -> Option<FileAttr> {
        self.cursor.skip_newlines();

        if !self.cursor.check(&TokenKind::HashBang) {
            return None;
        }
        self.cursor.advance(); // consume #!

        // Parse attribute name identifier
        let attr_kind = self.parse_attr_name(errors);

        match attr_kind {
            AttrKind::Target => {
                let mut attrs = ParsedAttrs::default();
                self.parse_target_attr(&mut attrs, errors, false);
                attrs.target.map(|t| FileAttr::Target {
                    os: t.os,
                    arch: t.arch,
                    family: t.family,
                    not_os: t.not_os,
                })
            }
            AttrKind::Cfg => {
                let mut attrs = ParsedAttrs::default();
                self.parse_cfg_attr(&mut attrs, errors, false);
                attrs.cfg.map(|c| FileAttr::Cfg {
                    debug: c.debug,
                    release: c.release,
                    not_debug: c.not_debug,
                    feature: c.feature,
                    not_feature: c.not_feature,
                })
            }
            AttrKind::Unknown => {
                // Error already reported by parse_attr_name
                self.skip_to_rparen_or_newline();
                None
            }
            other => {
                errors.push(ParseError {
                    code: ErrorCode::E1006,
                    message: format!(
                        "'{}' is not valid as a file-level attribute; \
                         only 'target' and 'cfg' are allowed",
                        other.as_str()
                    ),
                    span: self.cursor.previous_span(),
                    context: None,
                    help: Vec::new(),
                    severity: DiagnosticSeverity::Hard,
                });
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
mod tests {
    use super::*;
    use crate::parse;
    use ori_ir::StringInterner;

    fn parse_with_errors(source: &str) -> (crate::ParseOutput, StringInterner) {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let result = parse(&tokens, &interner);
        (result, interner)
    }

    #[test]
    fn test_parsed_attrs_token_capture() {
        // Create a parser and parse attributes directly to verify token capture
        let interner = StringInterner::new();
        let source = r#"#skip("reason") #compile_fail("error")"#;
        let tokens = ori_lexer::lex(source, &interner);
        let mut parser = crate::Parser::new(&tokens, &interner);
        let mut errors = Vec::new();

        let attrs = parser.parse_attributes(&mut errors);

        // Should have captured tokens
        assert!(attrs.has_tokens(), "Expected tokens to be captured");
        assert!(!attrs.token_range.is_empty());

        // Verify we can access the captured tokens
        let captured = tokens.get_range(attrs.token_range);
        assert!(captured.len() >= 2, "Should capture multiple tokens");

        // First token should be # (Hash)
        assert!(
            matches!(captured[0].kind, TokenKind::Hash),
            "First captured token should be #"
        );
    }

    #[test]
    fn test_parsed_attrs_no_tokens_when_no_attributes() {
        let interner = StringInterner::new();
        let source = r"def foo() -> int = 42";
        let tokens = ori_lexer::lex(source, &interner);
        let mut parser = crate::Parser::new(&tokens, &interner);
        let mut errors = Vec::new();

        let attrs = parser.parse_attributes(&mut errors);

        // Should NOT have captured tokens (no attributes)
        assert!(
            !attrs.has_tokens(),
            "Should not capture tokens when no attributes"
        );
        assert!(attrs.token_range.is_empty());
    }

    #[test]
    fn test_parse_skip_attribute() {
        let (result, _interner) = parse_with_errors(
            r#"
#[skip("not implemented")]
@test_example () -> void = print(msg: "test")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.skip_reason.is_some());
    }

    #[test]
    fn test_parse_compile_fail_attribute() {
        let (result, _interner) = parse_with_errors(
            r#"
#[compile_fail("type error")]
@test_should_fail () -> void = print(msg: "test")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.is_compile_fail());
        assert_eq!(test.expected_errors.len(), 1);
    }

    #[test]
    fn test_parse_fail_attribute() {
        let (result, _interner) = parse_with_errors(
            r#"
#[fail("assertion failed")]
@test_expect_failure () -> void = panic(msg: "expected failure")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.fail_expected.is_some());
    }

    #[test]
    fn test_parse_derive_attribute() {
        let (result, _interner) = parse_with_errors(
            r#"
#[derive(Eq, Clone)]
@test_with_derive () -> void = print(msg: "test")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_unknown_attribute() {
        let (result, _interner) = parse_with_errors(
            r#"
#[unknown("value")]
@test_unknown () -> void = print(msg: "test")
"#,
        );

        // Should have an error for unknown attribute
        assert!(result.has_errors());
        assert!(result
            .errors
            .iter()
            .any(|e| e.message.contains("unknown attribute")));
    }

    #[test]
    fn test_parse_attribute_missing_paren() {
        let (result, _interner) = parse_with_errors(
            r"
#[skip]
@test_bad () -> void = assert(cond: true)
",
        );

        // Should have an error for missing (
        assert!(result.has_errors());
    }

    #[test]
    fn test_parse_attribute_missing_string() {
        let (result, _interner) = parse_with_errors(
            r"
#[skip()]
@test_bad () -> void = assert(cond: true)
",
        );

        // Should have an error for missing string argument
        assert!(result.has_errors());
    }

    #[test]
    fn test_parse_multiple_attributes() {
        // Multiple attributes on same item isn't typical but parser should handle
        let (result, _interner) = parse_with_errors(
            r#"
#[skip("reason")]
#[fail("expected")]
@test_multi () -> void = print(msg: "test")
"#,
        );

        // Last attribute wins for each field
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
    }

    // Tests for new bracket-less syntax per grammar.ebnf

    #[test]
    fn test_parse_skip_attribute_no_brackets() {
        let (result, _interner) = parse_with_errors(
            r#"
#skip("not implemented")
@test_example () -> void = print(msg: "test")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.skip_reason.is_some());
    }

    #[test]
    fn test_parse_compile_fail_attribute_no_brackets() {
        let (result, _interner) = parse_with_errors(
            r#"
#compile_fail("type error")
@test_should_fail () -> void = print(msg: "test")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.is_compile_fail());
        assert_eq!(test.expected_errors.len(), 1);
    }

    #[test]
    fn test_parse_fail_attribute_no_brackets() {
        let (result, _interner) = parse_with_errors(
            r#"
#fail("assertion failed")
@test_expect_failure () -> void = panic(msg: "expected failure")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.fail_expected.is_some());
    }

    #[test]
    fn test_parse_derive_attribute_no_brackets() {
        let (result, _interner) = parse_with_errors(
            r"
#derive(Eq, Clone)
type Point = { x: int, y: int }
",
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_compile_fail_extended_no_brackets() {
        let (result, _interner) = parse_with_errors(
            r#"
#compile_fail(message: "type mismatch", code: "E2001")
@test_extended () -> void = print(msg: "test")
"#,
        );

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.is_compile_fail());
        assert_eq!(test.expected_errors.len(), 1);
    }

    // File-level attribute tests

    #[test]
    fn test_file_attr_target_parses() {
        let (result, _) = parse_with_errors("#!target(os: \"linux\")\n@main () -> void = ()");
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert!(result.module.file_attr.is_some());
    }

    #[test]
    fn test_file_attr_cfg_parses() {
        let (result, _) = parse_with_errors("#!cfg(debug)\n@main () -> void = ()");
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert!(result.module.file_attr.is_some());
    }

    #[test]
    fn test_file_attr_none_when_absent() {
        let (result, _) = parse_with_errors("@main () -> void = ()");
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert!(result.module.file_attr.is_none());
    }

    #[test]
    fn test_file_attr_does_not_consume_item_attr() {
        let (result, _) = parse_with_errors("#skip(\"reason\")\n@test_foo () -> void = ()");
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert!(
            result.module.file_attr.is_none(),
            "item-level #skip should not be consumed as file attribute"
        );
    }

    #[test]
    fn test_file_attr_invalid_kind_reports_error() {
        let (result, _) = parse_with_errors("#!derive(Eq)\n@main () -> void = ()");
        assert!(result.has_errors());
        assert!(result
            .errors
            .iter()
            .any(|e| e.message.contains("not valid as a file-level attribute")));
    }
}
