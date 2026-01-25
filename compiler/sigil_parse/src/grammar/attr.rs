//! Attribute parsing.
//!
//! This module extends Parser with methods for parsing attributes
//! like `#[skip("reason")]`, `#[compile_fail("error")]`, `#[fail("error")]`,
//! and `#[derive(Trait1, Trait2)]`.

use sigil_diagnostic::ErrorCode;
use sigil_ir::{Name, TokenKind};
use crate::{ParseError, Parser};

/// Parsed attributes for a function or test.
#[derive(Default, Clone, Debug)]
pub struct ParsedAttrs {
    /// Skip reason for `#[skip("reason")]`.
    pub skip_reason: Option<Name>,
    /// Expected error for `#[compile_fail("error")]`.
    pub compile_fail_expected: Option<Name>,
    /// Expected error for `#[fail("error")]`.
    pub fail_expected: Option<Name>,
    /// Derived traits for `#[derive(Trait1, Trait2)]` (future use).
    pub derive_traits: Vec<Name>,
}

impl ParsedAttrs {
    /// Returns true if no attributes are set.
    pub fn is_empty(&self) -> bool {
        self.skip_reason.is_none()
            && self.compile_fail_expected.is_none()
            && self.fail_expected.is_none()
            && self.derive_traits.is_empty()
    }
}

/// Kind of attribute being parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttrKind {
    Skip,
    CompileFail,
    Fail,
    Derive,
    Unknown,
}

impl AttrKind {
    fn as_str(self) -> &'static str {
        match self {
            AttrKind::Skip => "skip",
            AttrKind::CompileFail => "compile_fail",
            AttrKind::Fail => "fail",
            AttrKind::Derive => "derive",
            AttrKind::Unknown => "unknown",
        }
    }
}

impl<'a> Parser<'a> {
    /// Parse zero or more attributes: `#[attr("value")]` or `#[derive(Trait)]`.
    pub(crate) fn parse_attributes(
        &mut self,
        errors: &mut Vec<ParseError>,
    ) -> ParsedAttrs {
        let mut attrs = ParsedAttrs::default();

        while self.check(TokenKind::HashBracket) {
            self.advance(); // consume #[

            let attr_kind = self.parse_attr_name(errors);

            // For unknown attributes, skip to ] and continue
            if attr_kind == AttrKind::Unknown {
                self.skip_to_rbracket();
                continue;
            }

            match attr_kind {
                AttrKind::Derive => {
                    self.parse_derive_attr(&mut attrs, errors);
                }
                _ => {
                    self.parse_string_attr(attr_kind, &mut attrs, errors);
                }
            }

            self.skip_newlines();
        }

        attrs
    }

    /// Parse the attribute name and return its kind.
    fn parse_attr_name(&mut self, errors: &mut Vec<ParseError>) -> AttrKind {
        match self.current_kind() {
            TokenKind::Ident(name) => {
                let s = self.interner().lookup(name).to_owned();
                self.advance();
                match s.as_str() {
                    "skip" => AttrKind::Skip,
                    "compile_fail" => AttrKind::CompileFail,
                    "fail" => AttrKind::Fail,
                    "derive" => AttrKind::Derive,
                    _ => {
                        errors.push(ParseError::new(
                            ErrorCode::E1006,
                            format!("unknown attribute '{}'", s),
                            self.previous_span(),
                        ));
                        AttrKind::Unknown
                    }
                }
            }
            TokenKind::Skip => {
                self.advance();
                AttrKind::Skip
            }
            _ => {
                errors.push(ParseError::new(
                    ErrorCode::E1004,
                    format!("expected attribute name, found {:?}", self.current_kind()),
                    self.current_span(),
                ));
                AttrKind::Unknown
            }
        }
    }

    /// Parse a string-valued attribute like `#[skip("reason")]`.
    fn parse_string_attr(
        &mut self,
        attr_kind: AttrKind,
        attrs: &mut ParsedAttrs,
        errors: &mut Vec<ParseError>,
    ) {
        let attr_name_str = attr_kind.as_str();

        // Expect (
        if !self.check(TokenKind::LParen) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: format!("expected '(' after attribute name '{}'", attr_name_str),
                span: self.current_span(),
                context: None,
            });
            self.skip_to_rbracket();
            return;
        }
        self.advance(); // consume (

        // Parse string value
        let value = if let TokenKind::String(string_name) = self.current_kind() {
            self.advance();
            Some(string_name)
        } else {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: format!("attribute '{}' requires a string argument", attr_name_str),
                span: self.current_span(),
                context: None,
            });
            None
        };

        // Expect )
        if !self.check(TokenKind::RParen) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected ')' after attribute value".to_string(),
                span: self.current_span(),
                context: None,
            });
        } else {
            self.advance();
        }

        // Expect ]
        if !self.check(TokenKind::RBracket) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected ']' to close attribute".to_string(),
                span: self.current_span(),
                context: None,
            });
        } else {
            self.advance();
        }

        // Store the attribute
        if let Some(value) = value {
            match attr_kind {
                AttrKind::Skip => attrs.skip_reason = Some(value),
                AttrKind::CompileFail => attrs.compile_fail_expected = Some(value),
                AttrKind::Fail => attrs.fail_expected = Some(value),
                AttrKind::Derive | AttrKind::Unknown => {}
            }
        }
    }

    /// Parse a derive attribute like `#[derive(Eq, Clone)]`.
    fn parse_derive_attr(&mut self, attrs: &mut ParsedAttrs, errors: &mut Vec<ParseError>) {
        // Expect (
        if !self.check(TokenKind::LParen) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected '(' after 'derive'".to_string(),
                span: self.current_span(),
                context: None,
            });
            self.skip_to_rbracket();
            return;
        }
        self.advance(); // consume (

        // Parse trait list: Trait1, Trait2, ...
        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            match self.expect_ident() {
                Ok(name) => {
                    attrs.derive_traits.push(name);
                }
                Err(e) => {
                    errors.push(e);
                    self.skip_to_rbracket();
                    return;
                }
            }

            // Comma separator (optional before closing paren)
            if self.check(TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        // Expect )
        if !self.check(TokenKind::RParen) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected ')' after derive trait list".to_string(),
                span: self.current_span(),
                context: None,
            });
        } else {
            self.advance();
        }

        // Expect ]
        if !self.check(TokenKind::RBracket) {
            errors.push(ParseError {
                code: ErrorCode::E1006,
                message: "expected ']' to close attribute".to_string(),
                span: self.current_span(),
                context: None,
            });
        } else {
            self.advance();
        }
    }

    /// Skip tokens until we find a `]`.
    fn skip_to_rbracket(&mut self) {
        while !self.check(TokenKind::RBracket) && !self.is_at_end() {
            self.advance();
        }
        if self.check(TokenKind::RBracket) {
            self.advance();
        }
    }
}

#[cfg(test)]
mod tests {
    use sigil_ir::StringInterner;
    use sigil_lexer;
    use crate::parse;

    fn parse_with_errors(source: &str) -> (crate::ParseResult, StringInterner) {
        let interner = StringInterner::new();
        let tokens = sigil_lexer::lex(source, &interner);
        let result = parse(&tokens, &interner);
        (result, interner)
    }

    #[test]
    fn test_parse_skip_attribute() {
        let (result, _interner) = parse_with_errors(r#"
#[skip("not implemented")]
@test_example () -> void = print(msg: "test")
"#);

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.skip_reason.is_some());
    }

    #[test]
    fn test_parse_compile_fail_attribute() {
        let (result, _interner) = parse_with_errors(r#"
#[compile_fail("type error")]
@test_should_fail () -> void = print(msg: "test")
"#);

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.compile_fail_expected.is_some());
    }

    #[test]
    fn test_parse_fail_attribute() {
        let (result, _interner) = parse_with_errors(r#"
#[fail("assertion failed")]
@test_expect_failure () -> void = panic(msg: "expected failure")
"#);

        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        assert_eq!(result.module.tests.len(), 1);
        let test = &result.module.tests[0];
        assert!(test.fail_expected.is_some());
    }

    #[test]
    fn test_parse_derive_attribute() {
        // Note: derive is parsed but type definitions aren't implemented yet
        // This test verifies the parsing works for future use
        let (result, _interner) = parse_with_errors(r#"
#[derive(Eq, Clone)]
@test_with_derive () -> void = print(msg: "test")
"#);

        // The derive attribute is parsed but functions/tests don't use it
        // For now we just verify no parse errors
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_unknown_attribute() {
        let (result, _interner) = parse_with_errors(r#"
#[unknown("value")]
@test_unknown () -> void = print(msg: "test")
"#);

        // Should have an error for unknown attribute
        assert!(result.has_errors());
        assert!(result.errors.iter().any(|e| e.message.contains("unknown attribute")));
    }

    #[test]
    fn test_parse_attribute_missing_paren() {
        let (result, _interner) = parse_with_errors(r#"
#[skip]
@test_bad () -> void = assert(cond: true)
"#);

        // Should have an error for missing (
        assert!(result.has_errors());
    }

    #[test]
    fn test_parse_attribute_missing_string() {
        let (result, _interner) = parse_with_errors(r#"
#[skip()]
@test_bad () -> void = assert(cond: true)
"#);

        // Should have an error for missing string argument
        assert!(result.has_errors());
    }

    #[test]
    fn test_parse_multiple_attributes() {
        // Multiple attributes on same item isn't typical but parser should handle
        let (result, _interner) = parse_with_errors(r#"
#[skip("reason")]
#[fail("expected")]
@test_multi () -> void = print(msg: "test")
"#);

        // Last attribute wins for each field
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
    }
}
