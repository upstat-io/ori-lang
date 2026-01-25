//! Type parsing.
//!
//! This module extends Parser with methods for parsing type expressions.

use sigil_ir::{TokenKind, TypeId};
use crate::Parser;

impl<'a> Parser<'a> {
    /// Parse a type expression.
    /// Returns Some(TypeId) for primitive types, None for unknown/complex types.
    pub(crate) fn parse_type(&mut self) -> Option<TypeId> {
        if self.check_type_keyword() {
            let kind = self.current().kind.clone();
            self.advance();
            match kind {
                TokenKind::IntType => Some(TypeId::INT),
                TokenKind::FloatType => Some(TypeId::FLOAT),
                TokenKind::BoolType => Some(TypeId::BOOL),
                TokenKind::StrType => Some(TypeId::STR),
                TokenKind::CharType => Some(TypeId::CHAR),
                TokenKind::ByteType => Some(TypeId::BYTE),
                TokenKind::Void => Some(TypeId::VOID),
                TokenKind::NeverType => Some(TypeId::NEVER),
                _ => None,
            }
        } else if self.check(TokenKind::SelfUpper) {
            // Self type - used in trait/impl contexts
            self.advance();
            Some(TypeId::SELF_TYPE)
        } else if self.check_ident() {
            // Named type (possibly generic like Option<T>)
            self.advance();
            // Check for generic parameters
            self.parse_optional_generic_args();
            // TODO: Look up user-defined types
            None
        } else if self.check(TokenKind::LBracket) {
            // [T] list type
            self.advance(); // [
            self.parse_type(); // inner type
            if self.check(TokenKind::RBracket) {
                self.advance(); // ]
            }
            // TODO: Return proper list type
            None
        } else if self.check(TokenKind::LParen) {
            // (T, U) tuple or () unit or (T) -> U function type
            self.parse_paren_type()
        } else {
            None
        }
    }

    /// Parse optional generic arguments: `<T, U, ...>`
    fn parse_optional_generic_args(&mut self) {
        if !self.check(TokenKind::Lt) {
            return;
        }
        self.advance(); // <

        // Parse comma-separated type arguments
        while !self.check(TokenKind::Gt) && !self.is_at_end() {
            self.parse_type();
            if self.check(TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        if self.check(TokenKind::Gt) {
            self.advance(); // >
        }
    }

    /// Parse parenthesized types: unit `()`, tuple `(T, U)`, or function `(T) -> U`
    fn parse_paren_type(&mut self) -> Option<TypeId> {
        self.advance(); // (

        // Empty parens: () unit or () -> T function type
        if self.check(TokenKind::RParen) {
            self.advance(); // )
            // Check for -> (function type: () -> T)
            if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type();
                return None; // TODO: Return proper function type
            }
            return Some(TypeId::VOID); // () is unit/void
        }

        // Parse first element (could be tuple or function param)
        self.parse_type();

        // Collect remaining elements if tuple
        while self.check(TokenKind::Comma) {
            self.advance();
            if self.check(TokenKind::RParen) {
                break; // trailing comma
            }
            self.parse_type();
        }

        if self.check(TokenKind::RParen) {
            self.advance();
        }

        // Check for -> (function type)
        if self.check(TokenKind::Arrow) {
            self.advance();
            self.parse_type();
            // TODO: Return proper function type
        }

        None // TODO: Return proper tuple/function type
    }
}

#[cfg(test)]
mod tests {
    use sigil_ir::{StringInterner, TypeId};
    use sigil_lexer;
    use crate::Parser;

    fn parse_type_from_source(source: &str) -> Option<TypeId> {
        let interner = StringInterner::new();
        // Wrap in a function to get proper context for type parsing
        let full_source = format!("@test () -> {} = 0", source);
        let tokens = sigil_lexer::lex(&full_source, &interner);
        let mut parser = Parser::new(&tokens, &interner);

        // Skip to return type: @test () ->
        parser.advance(); // @
        parser.advance(); // test
        parser.advance(); // (
        parser.advance(); // )
        parser.advance(); // ->

        parser.parse_type()
    }

    #[test]
    fn test_parse_primitive_types() {
        assert_eq!(parse_type_from_source("int"), Some(TypeId::INT));
        assert_eq!(parse_type_from_source("float"), Some(TypeId::FLOAT));
        assert_eq!(parse_type_from_source("bool"), Some(TypeId::BOOL));
        assert_eq!(parse_type_from_source("str"), Some(TypeId::STR));
        assert_eq!(parse_type_from_source("char"), Some(TypeId::CHAR));
        assert_eq!(parse_type_from_source("byte"), Some(TypeId::BYTE));
        assert_eq!(parse_type_from_source("void"), Some(TypeId::VOID));
        assert_eq!(parse_type_from_source("Never"), Some(TypeId::NEVER));
    }

    #[test]
    fn test_parse_unit_type() {
        // () is void/unit
        assert_eq!(parse_type_from_source("()"), Some(TypeId::VOID));
    }

    #[test]
    fn test_parse_named_type() {
        // Named types currently return None (not yet fully implemented)
        assert_eq!(parse_type_from_source("MyType"), None);
    }

    #[test]
    fn test_parse_generic_type() {
        // Generic types like Option<int> currently return None
        assert_eq!(parse_type_from_source("Option<int>"), None);
        assert_eq!(parse_type_from_source("Result<int, str>"), None);
        assert_eq!(parse_type_from_source("Map<str, int>"), None);
    }

    #[test]
    fn test_parse_list_type() {
        // [T] list types currently return None
        assert_eq!(parse_type_from_source("[int]"), None);
        assert_eq!(parse_type_from_source("[str]"), None);
    }

    #[test]
    fn test_parse_tuple_type() {
        // (T, U) tuple types currently return None
        assert_eq!(parse_type_from_source("(int, str)"), None);
        assert_eq!(parse_type_from_source("(int, str, bool)"), None);
    }

    #[test]
    fn test_parse_function_type() {
        // Function types currently return None
        assert_eq!(parse_type_from_source("() -> int"), None);
        assert_eq!(parse_type_from_source("(int) -> str"), None);
        assert_eq!(parse_type_from_source("(int, str) -> bool"), None);
    }

    #[test]
    fn test_parse_nested_generic_type() {
        // Nested generics like Option<Result<int, str>>
        assert_eq!(parse_type_from_source("Option<Result<int, str>>"), None);
    }

    #[test]
    fn test_parse_self_type() {
        // Self type used in trait/impl contexts
        assert_eq!(parse_type_from_source("Self"), Some(TypeId::SELF_TYPE));
    }
}
