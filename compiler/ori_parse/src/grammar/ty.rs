//! Type parsing.
//!
//! This module extends Parser with methods for parsing type expressions.
//! Returns `ParsedType` which captures the full structure of type annotations.
//!
//! # Arena Allocation
//!
//! Types are allocated in the parser's arena. For recursive types (lists, maps,
//! functions, associated types), child types are allocated first and referenced
//! by ID. This enables flat storage without Box<ParsedType>.

use ori_ir::{ParsedType, ParsedTypeId, ParsedTypeRange, TokenKind, TypeId};

// Tag constants for type keyword dispatch (avoids cloning TokenKind).
use ori_ir::TokenKind as TK;

use crate::Parser;

impl Parser<'_> {
    /// Parse a type expression.
    /// Returns a `ParsedType` representing the full type structure.
    ///
    /// Recursive types use arena-allocated IDs for their children.
    pub(crate) fn parse_type(&mut self) -> Option<ParsedType> {
        if self.cursor.check_type_keyword() {
            // Read discriminant tag before advancing to avoid cloning the 16-byte TokenKind.
            let tag = self.cursor.current_tag();
            self.cursor.advance();
            match tag {
                TK::TAG_INT_TYPE => Some(ParsedType::primitive(TypeId::INT)),
                TK::TAG_FLOAT_TYPE => Some(ParsedType::primitive(TypeId::FLOAT)),
                TK::TAG_BOOL_TYPE => Some(ParsedType::primitive(TypeId::BOOL)),
                TK::TAG_STR_TYPE => Some(ParsedType::primitive(TypeId::STR)),
                TK::TAG_CHAR_TYPE => Some(ParsedType::primitive(TypeId::CHAR)),
                TK::TAG_BYTE_TYPE => Some(ParsedType::primitive(TypeId::BYTE)),
                TK::TAG_VOID => Some(ParsedType::primitive(TypeId::VOID)),
                TK::TAG_NEVER_TYPE => Some(ParsedType::primitive(TypeId::NEVER)),
                _ => None,
            }
        } else if self.cursor.check(&TokenKind::SelfUpper) {
            // Self type - used in trait/impl contexts
            self.cursor.advance();
            // Check for associated type access: Self.Item
            if self.cursor.check(&TokenKind::Dot) {
                self.cursor.advance(); // consume .
                if self.cursor.check_ident() {
                    let assoc_name = if let TokenKind::Ident(n) = &self.cursor.current().kind {
                        *n
                    } else {
                        return Some(ParsedType::SelfType);
                    };
                    self.cursor.advance();
                    // Allocate SelfType in arena for associated type base
                    let base_id = self.arena.alloc_parsed_type(ParsedType::SelfType);
                    Some(ParsedType::associated_type(base_id, assoc_name))
                } else {
                    Some(ParsedType::SelfType)
                }
            } else {
                Some(ParsedType::SelfType)
            }
        } else if self.cursor.check_ident() {
            // Named type (possibly generic like Option<T>)
            let name = if let TokenKind::Ident(n) = &self.cursor.current().kind {
                *n
            } else {
                return None;
            };
            self.cursor.advance();
            // Check for generic parameters
            let type_args = self.parse_optional_generic_args_range();
            let base_type = ParsedType::Named { name, type_args };

            // Check for associated type access: T.Item
            if self.cursor.check(&TokenKind::Dot) {
                self.cursor.advance(); // consume .
                if self.cursor.check_ident() {
                    let assoc_name = if let TokenKind::Ident(n) = &self.cursor.current().kind {
                        *n
                    } else {
                        return Some(base_type);
                    };
                    self.cursor.advance();
                    // Allocate base type in arena for associated type
                    let base_id = self.arena.alloc_parsed_type(base_type);
                    Some(ParsedType::associated_type(base_id, assoc_name))
                } else {
                    Some(base_type)
                }
            } else {
                Some(base_type)
            }
        } else if self.cursor.check(&TokenKind::LBracket) {
            // [T] list type or [T, max N] fixed-capacity list type
            self.cursor.advance(); // [
            let inner = self.parse_type()?;

            // Check for fixed-capacity syntax: [T, max N]
            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance(); // ,
                                       // Expect `max` identifier
                if let TokenKind::Ident(name) = self.cursor.current_kind() {
                    if self.cursor.interner().lookup(*name) == "max" {
                        self.cursor.advance(); // max
                                               // Parse capacity (integer literal)
                        if let TokenKind::Int(capacity) = *self.cursor.current_kind() {
                            self.cursor.advance(); // capacity
                            if self.cursor.check(&TokenKind::RBracket) {
                                self.cursor.advance(); // ]
                            }
                            let elem_id = self.arena.alloc_parsed_type(inner);
                            return Some(ParsedType::fixed_list(elem_id, capacity));
                        }
                    }
                }
                // If we get here, malformed fixed-capacity syntax - just return list
                if self.cursor.check(&TokenKind::RBracket) {
                    self.cursor.advance(); // ]
                }
                let elem_id = self.arena.alloc_parsed_type(inner);
                return Some(ParsedType::list(elem_id));
            }

            if self.cursor.check(&TokenKind::RBracket) {
                self.cursor.advance(); // ]
            }
            // Allocate element type in arena
            let elem_id = self.arena.alloc_parsed_type(inner);
            Some(ParsedType::list(elem_id))
        } else if self.cursor.check(&TokenKind::LBrace) {
            // {K: V} map type
            self.parse_map_type()
        } else if self.cursor.check(&TokenKind::LParen) {
            // (T, U) tuple or () unit or (T) -> U function type
            self.parse_paren_type()
        } else {
            None
        }
    }

    /// Parse a type and allocate it in the arena, returning its ID.
    ///
    /// This is a convenience method for cases where the parsed type
    /// needs to be stored as an ID (e.g., in lists, maps, functions).
    #[allow(
        dead_code,
        reason = "helper reserved for parsing nested types in future grammar rules"
    )]
    pub(crate) fn parse_type_id(&mut self) -> Option<ParsedTypeId> {
        let ty = self.parse_type()?;
        Some(self.arena.alloc_parsed_type(ty))
    }

    /// Parse optional generic arguments: `<T, U, ...>`
    /// Returns a range into the arena's type list storage.
    fn parse_optional_generic_args_range(&mut self) -> ParsedTypeRange {
        use crate::series::SeriesConfig;

        if !self.cursor.check(&TokenKind::Lt) {
            return ParsedTypeRange::EMPTY;
        }
        self.cursor.advance(); // <

        // Type arg lists use a Vec because nested generic args share the
        // same `parsed_type_lists` buffer (e.g., `Map<str, List<int>>`).
        let mut type_args: Vec<ParsedTypeId> = Vec::new();
        let _ = self.series_direct(&SeriesConfig::comma(TokenKind::Gt).no_newlines(), |p| {
            if p.cursor.check(&TokenKind::Gt) {
                return Ok(false);
            }
            if let Some(ty) = p.parse_type() {
                type_args.push(p.arena.alloc_parsed_type(ty));
                Ok(true)
            } else {
                Ok(false)
            }
        });

        if self.cursor.check(&TokenKind::Gt) {
            self.cursor.advance(); // >
        }

        self.arena.alloc_parsed_type_list(type_args)
    }

    /// Parse map type: {K: V}
    fn parse_map_type(&mut self) -> Option<ParsedType> {
        self.cursor.advance(); // {

        // Parse key type and allocate in arena
        let key = self.parse_type()?;
        let key_id = self.arena.alloc_parsed_type(key);

        // Expect colon
        if self.cursor.check(&TokenKind::Colon) {
            self.cursor.advance();
        }

        // Parse value type and allocate in arena
        let value = self.parse_type()?;
        let value_id = self.arena.alloc_parsed_type(value);

        // Expect closing brace
        if self.cursor.check(&TokenKind::RBrace) {
            self.cursor.advance();
        }

        Some(ParsedType::map(key_id, value_id))
    }

    /// Parse parenthesized types: unit `()`, tuple `(T, U)`, or function `(T) -> U`
    fn parse_paren_type(&mut self) -> Option<ParsedType> {
        self.cursor.advance(); // (

        // Empty parens: () unit or () -> T function type
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance(); // )
                                   // Check for -> (function type: () -> T)
            if self.cursor.check(&TokenKind::Arrow) {
                self.cursor.advance();
                let ret = self.parse_type()?;
                let ret_id = self.arena.alloc_parsed_type(ret);
                return Some(ParsedType::function(ParsedTypeRange::EMPTY, ret_id));
            }
            // () is unit (empty tuple)
            return Some(ParsedType::unit());
        }

        // Parse first element (could be tuple or function param)
        let mut element_ids = Vec::new();
        if let Some(first) = self.parse_type() {
            let id = self.arena.alloc_parsed_type(first);
            element_ids.push(id);
        }

        // Collect remaining elements if tuple
        while self.cursor.check(&TokenKind::Comma) {
            self.cursor.advance();
            if self.cursor.check(&TokenKind::RParen) {
                break; // trailing comma
            }
            if let Some(ty) = self.parse_type() {
                let id = self.arena.alloc_parsed_type(ty);
                element_ids.push(id);
            }
        }

        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();
        }

        // Check for -> (function type)
        if self.cursor.check(&TokenKind::Arrow) {
            self.cursor.advance();
            let ret = self.parse_type()?;
            let ret_id = self.arena.alloc_parsed_type(ret);
            let params = self.arena.alloc_parsed_type_list(element_ids);
            return Some(ParsedType::function(params, ret_id));
        }

        // If single element without arrow, it could be a parenthesized type or 1-tuple
        // We treat it as a tuple for consistency
        let elems = self.arena.alloc_parsed_type_list(element_ids);
        Some(ParsedType::tuple(elems))
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::{ExprArena, ParsedType, StringInterner, TypeId};

    use crate::Parser;

    /// Parse a type from source, returning the type and the arena for lookups.
    fn parse_type_with_arena(source: &str) -> (Option<ParsedType>, ExprArena) {
        let interner = StringInterner::new();
        // Wrap in a function to get proper context for type parsing
        let full_source = format!("@test () -> {source} = 0");
        let tokens = ori_lexer::lex(&full_source, &interner);
        let mut parser = Parser::new(&tokens, &interner);

        // Skip to return type: @test () ->
        parser.cursor.advance(); // @
        parser.cursor.advance(); // test
        parser.cursor.advance(); // (
        parser.cursor.advance(); // )
        parser.cursor.advance(); // ->

        let ty = parser.parse_type();
        let arena = parser.take_arena();
        (ty, arena)
    }

    #[test]
    fn test_parse_primitive_types() {
        let (ty, _) = parse_type_with_arena("int");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::INT)));

        let (ty, _) = parse_type_with_arena("float");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::FLOAT)));

        let (ty, _) = parse_type_with_arena("bool");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::BOOL)));

        let (ty, _) = parse_type_with_arena("str");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::STR)));

        let (ty, _) = parse_type_with_arena("char");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::CHAR)));

        let (ty, _) = parse_type_with_arena("byte");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::BYTE)));

        let (ty, _) = parse_type_with_arena("void");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::VOID)));

        let (ty, _) = parse_type_with_arena("Never");
        assert_eq!(ty, Some(ParsedType::primitive(TypeId::NEVER)));
    }

    #[test]
    fn test_parse_unit_type() {
        // () is unit (empty tuple)
        let (ty, _) = parse_type_with_arena("()");
        assert!(matches!(ty, Some(ParsedType::Tuple(ref v)) if v.is_empty()));
    }

    #[test]
    fn test_parse_named_type() {
        let (ty, _) = parse_type_with_arena("MyType");
        assert!(matches!(
            ty,
            Some(ParsedType::Named { type_args, .. }) if type_args.is_empty()
        ));
    }

    #[test]
    fn test_parse_generic_type() {
        // Generic types like Option<int>
        let (ty, arena) = parse_type_with_arena("Option<int>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 1);
                let ids = arena.get_parsed_type_list(type_args);
                assert_eq!(
                    *arena.get_parsed_type(ids[0]),
                    ParsedType::primitive(TypeId::INT)
                );
            }
            _ => panic!("expected Named with type args"),
        }

        // Result<int, str>
        let (ty, arena) = parse_type_with_arena("Result<int, str>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 2);
                let ids = arena.get_parsed_type_list(type_args);
                assert_eq!(
                    *arena.get_parsed_type(ids[0]),
                    ParsedType::primitive(TypeId::INT)
                );
                assert_eq!(
                    *arena.get_parsed_type(ids[1]),
                    ParsedType::primitive(TypeId::STR)
                );
            }
            _ => panic!("expected Named with 2 type args"),
        }
    }

    #[test]
    fn test_parse_list_type() {
        let (ty, arena) = parse_type_with_arena("[int]");
        match ty {
            Some(ParsedType::List(inner_id)) => {
                assert_eq!(
                    *arena.get_parsed_type(inner_id),
                    ParsedType::primitive(TypeId::INT)
                );
            }
            _ => panic!("expected List"),
        }

        let (ty, arena) = parse_type_with_arena("[str]");
        match ty {
            Some(ParsedType::List(inner_id)) => {
                assert_eq!(
                    *arena.get_parsed_type(inner_id),
                    ParsedType::primitive(TypeId::STR)
                );
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_tuple_type() {
        let (ty, arena) = parse_type_with_arena("(int, str)");
        match ty {
            Some(ParsedType::Tuple(elems)) => {
                assert_eq!(elems.len(), 2);
                let ids = arena.get_parsed_type_list(elems);
                assert_eq!(
                    *arena.get_parsed_type(ids[0]),
                    ParsedType::primitive(TypeId::INT)
                );
                assert_eq!(
                    *arena.get_parsed_type(ids[1]),
                    ParsedType::primitive(TypeId::STR)
                );
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn test_parse_function_type() {
        let (ty, arena) = parse_type_with_arena("() -> int");
        match ty {
            Some(ParsedType::Function { params, ret }) => {
                assert!(params.is_empty());
                assert_eq!(
                    *arena.get_parsed_type(ret),
                    ParsedType::primitive(TypeId::INT)
                );
            }
            _ => panic!("expected Function"),
        }

        let (ty, arena) = parse_type_with_arena("(int) -> str");
        match ty {
            Some(ParsedType::Function { params, ret }) => {
                assert_eq!(params.len(), 1);
                let param_ids = arena.get_parsed_type_list(params);
                assert_eq!(
                    *arena.get_parsed_type(param_ids[0]),
                    ParsedType::primitive(TypeId::INT)
                );
                assert_eq!(
                    *arena.get_parsed_type(ret),
                    ParsedType::primitive(TypeId::STR)
                );
            }
            _ => panic!("expected Function"),
        }

        let (ty, arena) = parse_type_with_arena("(int, str) -> bool");
        match ty {
            Some(ParsedType::Function { params, ret }) => {
                assert_eq!(params.len(), 2);
                assert_eq!(
                    *arena.get_parsed_type(ret),
                    ParsedType::primitive(TypeId::BOOL)
                );
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_parse_nested_generic_type() {
        // Nested generics like Option<Result<int, str>>
        let (ty, arena) = parse_type_with_arena("Option<Result<int, str>>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 1);
                let ids = arena.get_parsed_type_list(type_args);
                match arena.get_parsed_type(ids[0]) {
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
    fn test_parse_double_nested_generic_type() {
        // Double nested generics: Result<Result<T, E>, E>
        // This was previously broken because >> was lexed as a single Shr token.
        // Now the lexer produces individual > tokens, enabling correct parsing.
        let (ty, arena) = parse_type_with_arena("Result<Result<int, str>, str>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 2, "Expected 2 type args for outer Result");
                let outer_ids = arena.get_parsed_type_list(type_args);
                // First arg should be Result<int, str>
                match arena.get_parsed_type(outer_ids[0]) {
                    ParsedType::Named {
                        type_args: inner, ..
                    } => {
                        assert_eq!(inner.len(), 2, "Expected 2 type args for inner Result");
                        let inner_ids = arena.get_parsed_type_list(*inner);
                        assert_eq!(
                            *arena.get_parsed_type(inner_ids[0]),
                            ParsedType::primitive(TypeId::INT)
                        );
                        assert_eq!(
                            *arena.get_parsed_type(inner_ids[1]),
                            ParsedType::primitive(TypeId::STR)
                        );
                    }
                    _ => panic!("expected inner Named (Result<int, str>)"),
                }
                // Second arg should be str
                assert_eq!(
                    *arena.get_parsed_type(outer_ids[1]),
                    ParsedType::primitive(TypeId::STR)
                );
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_parse_triple_nested_generic_type() {
        // Triple nested: Option<Result<Result<int, str>, str>>
        let (ty, arena) = parse_type_with_arena("Option<Result<Result<int, str>, str>>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 1, "Expected 1 type arg for Option");
                let outer_ids = arena.get_parsed_type_list(type_args);
                match arena.get_parsed_type(outer_ids[0]) {
                    ParsedType::Named {
                        type_args: inner, ..
                    } => {
                        assert_eq!(inner.len(), 2, "Expected 2 type args for outer Result");
                        let inner_ids = arena.get_parsed_type_list(*inner);
                        // First arg should be Result<int, str>
                        match arena.get_parsed_type(inner_ids[0]) {
                            ParsedType::Named {
                                type_args: deepest, ..
                            } => {
                                assert_eq!(
                                    deepest.len(),
                                    2,
                                    "Expected 2 type args for inner Result"
                                );
                            }
                            _ => panic!("expected innermost Named"),
                        }
                    }
                    _ => panic!("expected inner Named (Result<...>)"),
                }
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_parse_self_type() {
        let (ty, _) = parse_type_with_arena("Self");
        assert_eq!(ty, Some(ParsedType::SelfType));
    }

    #[test]
    fn test_parse_list_of_generic() {
        // [Option<int>]
        let (ty, arena) = parse_type_with_arena("[Option<int>]");
        match ty {
            Some(ParsedType::List(inner_id)) => match arena.get_parsed_type(inner_id) {
                ParsedType::Named { type_args, .. } => {
                    assert_eq!(type_args.len(), 1);
                }
                _ => panic!("expected Named inside List"),
            },
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_self_associated_type() {
        // Self.Item - associated type access on Self
        let (ty, arena) = parse_type_with_arena("Self.Item");
        match ty {
            Some(ParsedType::AssociatedType { base, assoc_name }) => {
                assert_eq!(*arena.get_parsed_type(base), ParsedType::SelfType);
                // Note: assoc_name is a Name, we just verify it was parsed
                let _ = assoc_name;
            }
            _ => panic!("expected AssociatedType, got {ty:?}"),
        }
    }

    #[test]
    fn test_parse_generic_associated_type() {
        // T.Item - associated type access on a type variable
        let (ty, arena) = parse_type_with_arena("T.Item");
        match ty {
            Some(ParsedType::AssociatedType { base, assoc_name }) => {
                match arena.get_parsed_type(base) {
                    ParsedType::Named { type_args, .. } => {
                        assert!(type_args.is_empty());
                    }
                    _ => panic!("expected Named as base"),
                }
                let _ = assoc_name;
            }
            _ => panic!("expected AssociatedType, got {ty:?}"),
        }
    }

    #[test]
    fn test_parse_option_of_associated_type() {
        // Option<Self.Item> - associated type inside generic
        let (ty, arena) = parse_type_with_arena("Option<Self.Item>");
        match ty {
            Some(ParsedType::Named { type_args, .. }) => {
                assert_eq!(type_args.len(), 1);
                let ids = arena.get_parsed_type_list(type_args);
                match arena.get_parsed_type(ids[0]) {
                    ParsedType::AssociatedType { base, .. } => {
                        assert_eq!(*arena.get_parsed_type(*base), ParsedType::SelfType);
                    }
                    _ => panic!("expected AssociatedType as type arg"),
                }
            }
            _ => panic!("expected Named"),
        }
    }
}
