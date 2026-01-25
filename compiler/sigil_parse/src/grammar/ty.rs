//! Type parsing.
//!
//! This module extends Parser with methods for parsing type expressions.
//! Returns `ParsedType` which captures the full structure of type annotations.

use sigil_ir::{ParsedType, TokenKind, TypeId};

use crate::Parser;

impl Parser<'_> {
    /// Parse a type expression.
    /// Returns a `ParsedType` representing the full type structure.
    pub(crate) fn parse_type(&mut self) -> Option<ParsedType> {
        if self.check_type_keyword() {
            let kind = self.current().kind.clone();
            self.advance();
            match kind {
                TokenKind::IntType => Some(ParsedType::primitive(TypeId::INT)),
                TokenKind::FloatType => Some(ParsedType::primitive(TypeId::FLOAT)),
                TokenKind::BoolType => Some(ParsedType::primitive(TypeId::BOOL)),
                TokenKind::StrType => Some(ParsedType::primitive(TypeId::STR)),
                TokenKind::CharType => Some(ParsedType::primitive(TypeId::CHAR)),
                TokenKind::ByteType => Some(ParsedType::primitive(TypeId::BYTE)),
                TokenKind::Void => Some(ParsedType::primitive(TypeId::VOID)),
                TokenKind::NeverType => Some(ParsedType::primitive(TypeId::NEVER)),
                _ => None,
            }
        } else if self.check(&TokenKind::SelfUpper) {
            // Self type - used in trait/impl contexts
            self.advance();
            Some(ParsedType::SelfType)
        } else if self.check_ident() {
            // Named type (possibly generic like Option<T>)
            let name = if let TokenKind::Ident(n) = &self.current().kind {
                *n
            } else {
                return None;
            };
            self.advance();
            // Check for generic parameters
            let type_args = self.parse_optional_generic_args_full();
            Some(ParsedType::Named { name, type_args })
        } else if self.check(&TokenKind::LBracket) {
            // [T] list type
            self.advance(); // [
            let inner = self.parse_type()?;
            if self.check(&TokenKind::RBracket) {
                self.advance(); // ]
            }
            Some(ParsedType::list(inner))
        } else if self.check(&TokenKind::LBrace) {
            // {K: V} map type
            self.parse_map_type()
        } else if self.check(&TokenKind::LParen) {
            // (T, U) tuple or () unit or (T) -> U function type
            self.parse_paren_type()
        } else {
            None
        }
    }

    /// Parse optional generic arguments: `<T, U, ...>`
    /// Returns an empty Vec if no generic arguments are present.
    fn parse_optional_generic_args_full(&mut self) -> Vec<ParsedType> {
        if !self.check(&TokenKind::Lt) {
            return Vec::new();
        }
        self.advance(); // <

        let mut args = Vec::new();

        // Parse comma-separated type arguments
        while !self.check(&TokenKind::Gt) && !self.is_at_end() {
            if let Some(ty) = self.parse_type() {
                args.push(ty);
            }
            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        if self.check(&TokenKind::Gt) {
            self.advance(); // >
        }

        args
    }

    /// Parse map type: {K: V}
    fn parse_map_type(&mut self) -> Option<ParsedType> {
        self.advance(); // {

        // Parse key type
        let key = self.parse_type()?;

        // Expect colon
        if self.check(&TokenKind::Colon) {
            self.advance();
        }

        // Parse value type
        let value = self.parse_type()?;

        // Expect closing brace
        if self.check(&TokenKind::RBrace) {
            self.advance();
        }

        Some(ParsedType::map(key, value))
    }

    /// Parse parenthesized types: unit `()`, tuple `(T, U)`, or function `(T) -> U`
    fn parse_paren_type(&mut self) -> Option<ParsedType> {
        self.advance(); // (

        // Empty parens: () unit or () -> T function type
        if self.check(&TokenKind::RParen) {
            self.advance(); // )
            // Check for -> (function type: () -> T)
            if self.check(&TokenKind::Arrow) {
                self.advance();
                let ret = self.parse_type()?;
                return Some(ParsedType::function(Vec::new(), ret));
            }
            // () is unit (empty tuple)
            return Some(ParsedType::unit());
        }

        // Parse first element (could be tuple or function param)
        let mut elements = Vec::new();
        if let Some(first) = self.parse_type() {
            elements.push(first);
        }

        // Collect remaining elements if tuple
        while self.check(&TokenKind::Comma) {
            self.advance();
            if self.check(&TokenKind::RParen) {
                break; // trailing comma
            }
            if let Some(ty) = self.parse_type() {
                elements.push(ty);
            }
        }

        if self.check(&TokenKind::RParen) {
            self.advance();
        }

        // Check for -> (function type)
        if self.check(&TokenKind::Arrow) {
            self.advance();
            let ret = self.parse_type()?;
            return Some(ParsedType::function(elements, ret));
        }

        // If single element without arrow, it could be a parenthesized type or 1-tuple
        // We treat it as a tuple for consistency
        Some(ParsedType::tuple(elements))
    }
}

#[cfg(test)]
mod tests {
    use sigil_ir::{ParsedType, StringInterner, TypeId};
    use sigil_lexer;

    use crate::Parser;

    fn parse_type_from_source(source: &str) -> Option<ParsedType> {
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
        assert_eq!(
            parse_type_from_source("int"),
            Some(ParsedType::primitive(TypeId::INT))
        );
        assert_eq!(
            parse_type_from_source("float"),
            Some(ParsedType::primitive(TypeId::FLOAT))
        );
        assert_eq!(
            parse_type_from_source("bool"),
            Some(ParsedType::primitive(TypeId::BOOL))
        );
        assert_eq!(
            parse_type_from_source("str"),
            Some(ParsedType::primitive(TypeId::STR))
        );
        assert_eq!(
            parse_type_from_source("char"),
            Some(ParsedType::primitive(TypeId::CHAR))
        );
        assert_eq!(
            parse_type_from_source("byte"),
            Some(ParsedType::primitive(TypeId::BYTE))
        );
        assert_eq!(
            parse_type_from_source("void"),
            Some(ParsedType::primitive(TypeId::VOID))
        );
        assert_eq!(
            parse_type_from_source("Never"),
            Some(ParsedType::primitive(TypeId::NEVER))
        );
    }

    #[test]
    fn test_parse_unit_type() {
        // () is unit (empty tuple)
        let ty = parse_type_from_source("()");
        assert!(matches!(ty, Some(ParsedType::Tuple(ref v)) if v.is_empty()));
    }

    #[test]
    fn test_parse_named_type() {
        let ty = parse_type_from_source("MyType");
        assert!(matches!(
            ty,
            Some(ParsedType::Named { type_args, .. }) if type_args.is_empty()
        ));
    }

    #[test]
    fn test_parse_generic_type() {
        // Generic types like Option<int>
        let ty = parse_type_from_source("Option<int>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 1);
                assert_eq!(type_args[0], ParsedType::primitive(TypeId::INT));
            }
            _ => panic!("expected Named with type args"),
        }

        // Result<int, str>
        let ty = parse_type_from_source("Result<int, str>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 2);
                assert_eq!(type_args[0], ParsedType::primitive(TypeId::INT));
                assert_eq!(type_args[1], ParsedType::primitive(TypeId::STR));
            }
            _ => panic!("expected Named with 2 type args"),
        }
    }

    #[test]
    fn test_parse_list_type() {
        let ty = parse_type_from_source("[int]");
        match ty {
            Some(ParsedType::List(inner)) => {
                assert_eq!(*inner, ParsedType::primitive(TypeId::INT));
            }
            _ => panic!("expected List"),
        }

        let ty = parse_type_from_source("[str]");
        match ty {
            Some(ParsedType::List(inner)) => {
                assert_eq!(*inner, ParsedType::primitive(TypeId::STR));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_tuple_type() {
        let ty = parse_type_from_source("(int, str)");
        match ty {
            Some(ParsedType::Tuple(elems)) => {
                assert_eq!(elems.len(), 2);
                assert_eq!(elems[0], ParsedType::primitive(TypeId::INT));
                assert_eq!(elems[1], ParsedType::primitive(TypeId::STR));
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn test_parse_function_type() {
        let ty = parse_type_from_source("() -> int");
        match ty {
            Some(ParsedType::Function { params, ret }) => {
                assert!(params.is_empty());
                assert_eq!(*ret, ParsedType::primitive(TypeId::INT));
            }
            _ => panic!("expected Function"),
        }

        let ty = parse_type_from_source("(int) -> str");
        match ty {
            Some(ParsedType::Function { params, ret }) => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0], ParsedType::primitive(TypeId::INT));
                assert_eq!(*ret, ParsedType::primitive(TypeId::STR));
            }
            _ => panic!("expected Function"),
        }

        let ty = parse_type_from_source("(int, str) -> bool");
        match ty {
            Some(ParsedType::Function { params, ret }) => {
                assert_eq!(params.len(), 2);
                assert_eq!(*ret, ParsedType::primitive(TypeId::BOOL));
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_parse_nested_generic_type() {
        // Nested generics like Option<Result<int, str>>
        let ty = parse_type_from_source("Option<Result<int, str>>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 1);
                match &type_args[0] {
                    ParsedType::Named {
                        type_args: inner, ..
                    } => {
                        assert_eq!(inner.len(), 2);
                    }
                    _ => panic!("expected inner Named"),
                }
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_parse_self_type() {
        let ty = parse_type_from_source("Self");
        assert_eq!(ty, Some(ParsedType::SelfType));
    }

    #[test]
    fn test_parse_list_of_generic() {
        // [Option<int>]
        let ty = parse_type_from_source("[Option<int>]");
        match ty {
            Some(ParsedType::List(inner)) => match inner.as_ref() {
                ParsedType::Named { type_args, .. } => {
                    assert_eq!(type_args.len(), 1);
                }
                _ => panic!("expected Named inside List"),
            },
            _ => panic!("expected List"),
        }
    }
}
