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

mod compile_fail;
mod conditional;
mod repr;
mod simple;

use crate::{ParseError, Parser};
use ori_diagnostic::ErrorCode;
use ori_ir::{FileAttr, Name, TokenCapture, TokenKind};

/// Parsed attributes for a function or test.
///
/// Contains both the semantic attribute values and an optional token capture
/// for formatters and IDE features.
#[derive(Default, Clone, Debug)]
pub struct ParsedAttrs {
    /// Skip reason for `#skip("reason")`.
    pub skip_reason: Option<Name>,
    /// Expected compilation errors (multiple allowed).
    pub expected_errors: Vec<ori_ir::ExpectedError>,
    /// Expected error for `#fail("error")`.
    pub fail_expected: Option<Name>,
    /// Derived traits for `#derive(Trait1, Trait2)`.
    pub derive_traits: Vec<Name>,
    /// Repr attributes for `#repr("c")`, `#repr("packed")`, etc.
    ///
    /// Multiple `#repr` may be stacked (e.g., `#repr("c") #repr("aligned", 16)`).
    pub repr_attrs: Vec<ReprAttr>,
    /// Target conditional compilation for `#target(os: "linux")`.
    pub target: Option<ori_ir::TargetAttr>,
    /// Config conditional compilation for `#cfg(debug)`.
    pub cfg: Option<ori_ir::CfgAttr>,
    /// FBIP enforcement annotation: `#fbip`.
    pub is_fbip: bool,

    /// Token range covering all attributes (for formatters/IDE).
    ///
    /// This captures the indices of tokens from the first `#` to the last
    /// attribute closing token. Use `TokenList::get_range()` to access
    /// the actual tokens.
    pub token_range: TokenCapture,
}

/// Representation attribute values.
///
/// Converted to [`ori_ir::ReprAttrKind`] during type declaration parsing.
#[derive(Clone, Debug)]
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
            && self.repr_attrs.is_empty()
            && self.target.is_none()
            && self.cfg.is_none()
            && !self.is_fbip
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
    Fbip,
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
            AttrKind::Fbip => "fbip",
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
                AttrKind::Fbip => {
                    // #fbip is a bare flag — no arguments.
                    attrs.is_fbip = true;
                    if uses_brackets {
                        self.finish_attr_bracket(uses_brackets, errors);
                    }
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
                    "fbip" => AttrKind::Fbip,
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

    // Recovery helpers — used by submodules via `self.method()`.

    /// Skip to the end of an attribute during error recovery.
    ///
    /// Bracket-style (`#[...]`) skips to `]`; bracketless (`#...`) skips to `)` or newline.
    fn skip_to_attr_end(&mut self, uses_brackets: bool) {
        if uses_brackets {
            self.skip_to_rbracket();
        } else {
            self.skip_to_rparen_or_newline();
        }
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

    /// Finish the closing `]` bracket for old-style bracket attributes.
    ///
    /// No-op if `uses_brackets` is false.
    fn finish_attr_bracket(&mut self, uses_brackets: bool, errors: &mut Vec<ParseError>) {
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
