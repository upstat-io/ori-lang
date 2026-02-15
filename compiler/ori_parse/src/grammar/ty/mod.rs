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

use ori_diagnostic::ErrorCode;
use ori_ir::{ParsedType, ParsedTypeId, ParsedTypeRange, TokenKind, TypeId};

// Tag constants for type keyword dispatch (avoids cloning TokenKind).
use ori_ir::TokenKind as TK;

use crate::error::ParseError;
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
            let result = if self.cursor.check(&TokenKind::Dot) {
                self.cursor.advance(); // consume .
                if let TokenKind::Ident(n) = self.cursor.current_kind() {
                    let assoc_name = *n;
                    self.cursor.advance();
                    let base_id = self.arena.alloc_parsed_type(base_type);
                    ParsedType::associated_type(base_id, assoc_name)
                } else {
                    base_type
                }
            } else {
                base_type
            };

            // Check for bounded trait object: Trait1 + Trait2 [+ Trait3 ...]
            if self.cursor.check(&TokenKind::Plus) {
                let first_id = self.arena.alloc_parsed_type(result);
                let mut bound_ids = vec![first_id];
                while self.cursor.check(&TokenKind::Plus) {
                    self.cursor.advance(); // consume +
                                           // Parse next bound as a named type (ident + optional generics)
                    if self.cursor.check_ident() {
                        let bound_name = if let TokenKind::Ident(n) = &self.cursor.current().kind {
                            *n
                        } else {
                            break;
                        };
                        self.cursor.advance();
                        let bound_args = self.parse_optional_generic_args_range();
                        let bound_type = ParsedType::Named {
                            name: bound_name,
                            type_args: bound_args,
                        };
                        bound_ids.push(self.arena.alloc_parsed_type(bound_type));
                    } else {
                        break;
                    }
                }
                let bounds = self.arena.alloc_parsed_type_list(bound_ids);
                Some(ParsedType::trait_bounds(bounds))
            } else {
                Some(result)
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
                    if *name == self.known.max {
                        self.cursor.advance(); // max
                                               // Parse capacity as const expression ($N, 42, $N + 1)
                        if let Ok(capacity_expr) = self.parse_non_comparison_expr().into_result() {
                            if self.cursor.check(&TokenKind::RBracket) {
                                self.cursor.advance(); // ]
                            }
                            let elem_id = self.arena.alloc_parsed_type(inner);
                            return Some(ParsedType::fixed_list(elem_id, capacity_expr));
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
        } else if self.cursor.check(&TokenKind::Amp) {
            // &T — borrowed references are reserved for future use.
            // Consume & and try to parse the inner type for recovery.
            let amp_span = self.cursor.current().span;
            self.cursor.advance(); // consume &
            self.deferred_errors.push(ParseError::new(
                ErrorCode::E1001,
                "borrowed references (`&T`) are reserved for a future version of Ori",
                amp_span,
            ));
            // Try to parse the inner type so parsing can recover
            self.parse_type().or(Some(ParsedType::Infer))
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
    pub(crate) fn parse_optional_generic_args_range(&mut self) -> ParsedTypeRange {
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
            let tag = p.cursor.current_tag();
            if tag == TK::TAG_DOLLAR || tag == TK::TAG_INT {
                // Const expression in type argument position: $N, $N + 1, 42
                let expr_id = p.parse_non_comparison_expr().into_result()?;
                type_args.push(p.arena.alloc_parsed_type(ParsedType::const_expr(expr_id)));
                Ok(true)
            } else if let Some(ty) = p.parse_type() {
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
mod tests;
