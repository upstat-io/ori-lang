//! Type parsing.
//!
//! This module extends Parser with methods for parsing type expressions.
//! Returns `ParsedType` which captures the full structure of type annotations.
//!
//! # Arena Allocation
//!
//! Types are allocated in the parser's arena. For recursive types (lists, maps,
//! functions, associated types), child types are allocated first and referenced
//! by ID. This enables flat storage without `Box<ParsedType>`.

use ori_diagnostic::ErrorCode;
use ori_ir::{Name, ParsedType, ParsedTypeId, ParsedTypeRange, TokenKind, TypeId};

// Tag constants for type keyword dispatch (avoids cloning TokenKind).
use ori_ir::TokenKind as TK;

use crate::error::ParseError;
use crate::Parser;

impl Parser<'_> {
    /// Parse a type expression.
    /// Returns a `ParsedType` representing the full type structure.
    ///
    /// Recursive types use arena-allocated IDs for their children.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn parse_type(&mut self) -> Option<ParsedType> {
        if self.cursor.check_type_keyword() {
            self.parse_primitive_type()
        } else if self.cursor.check(&TokenKind::SelfUpper) {
            Some(self.parse_self_type())
        } else if self.cursor.check_ident() {
            self.parse_named_type()
        } else if self.cursor.check(&TokenKind::LBracket) {
            self.parse_list_type()
        } else if self.cursor.check(&TokenKind::LBrace) {
            self.parse_map_type()
        } else if self.cursor.check(&TokenKind::LParen) {
            self.parse_paren_type()
        } else if self.cursor.check(&TokenKind::Amp) {
            Some(self.parse_reserved_borrowed_type())
        } else {
            None
        }
    }

    fn parse_primitive_type(&mut self) -> Option<ParsedType> {
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
    }

    fn parse_self_type(&mut self) -> ParsedType {
        self.cursor.advance();
        if !self.cursor.check(&TokenKind::Dot) {
            return ParsedType::SelfType;
        }
        self.cursor.advance();
        let TokenKind::Ident(associated) = self.cursor.current_kind() else {
            return ParsedType::SelfType;
        };
        let associated = *associated;
        self.cursor.advance();
        let type_args = self.parse_optional_generic_args_range();
        let base = self.arena.alloc_parsed_type(ParsedType::SelfType);
        ParsedType::associated_type(base, associated, type_args)
    }

    fn parse_named_type(&mut self) -> Option<ParsedType> {
        let TokenKind::Ident(name) = self.cursor.current_kind() else {
            return None;
        };
        let name = *name;
        self.cursor.advance();
        // `type = type_path [ type_args ]` and
        // `type_path = identifier { "." identifier }` (Spec: grammar.ebnf),
        // so the dotted path is consumed in full FIRST and the generic args
        // bind to its terminal segment: `a.b.C<int>` puts `<int>` on `C`.
        let mut segments: Vec<Name> = Vec::new();
        while self.cursor.check(&TokenKind::Dot) {
            self.cursor.advance();
            let TokenKind::Ident(associated) = self.cursor.current_kind() else {
                break;
            };
            segments.push(*associated);
            self.cursor.advance();
        }
        let type_args = self.parse_optional_generic_args_range();
        let mut head = ParsedType::Named {
            name,
            type_args: if segments.is_empty() {
                type_args
            } else {
                ParsedTypeRange::EMPTY
            },
        };
        let last = segments.len().saturating_sub(1);
        for (index, associated) in segments.into_iter().enumerate() {
            let base = self.arena.alloc_parsed_type(head);
            let args = if index == last {
                type_args
            } else {
                ParsedTypeRange::EMPTY
            };
            head = ParsedType::associated_type(base, associated, args);
        }
        if !self.cursor.check(&TokenKind::Plus) {
            return Some(head);
        }

        let first = self.arena.alloc_parsed_type(head);
        let mut bounds = vec![first];
        while self.cursor.check(&TokenKind::Plus) {
            self.cursor.advance();
            let TokenKind::Ident(name) = self.cursor.current_kind() else {
                break;
            };
            let name = *name;
            self.cursor.advance();
            let type_args = self.parse_optional_generic_args_range();
            bounds.push(
                self.arena
                    .alloc_parsed_type(ParsedType::Named { name, type_args }),
            );
        }
        let bounds = self.arena.alloc_parsed_type_list(bounds);
        Some(ParsedType::trait_bounds(bounds))
    }

    fn parse_list_type(&mut self) -> Option<ParsedType> {
        self.cursor.advance();
        let inner = self.parse_type()?;
        if self.cursor.check(&TokenKind::Comma) {
            self.cursor.advance();
            if matches!(self.cursor.current_kind(), TokenKind::Ident(name) if *name == self.known.max)
            {
                self.cursor.advance();
                if let Ok(capacity) = self.parse_non_comparison_expr().into_result() {
                    self.consume_type_delimiter_if(&TokenKind::RBracket);
                    let element = self.arena.alloc_parsed_type(inner);
                    return Some(ParsedType::fixed_list(element, capacity));
                }
            }
            self.consume_type_delimiter_if(&TokenKind::RBracket);
            let element = self.arena.alloc_parsed_type(inner);
            return Some(ParsedType::list(element));
        }
        self.consume_type_delimiter_if(&TokenKind::RBracket);
        let element = self.arena.alloc_parsed_type(inner);
        Some(ParsedType::list(element))
    }

    fn parse_reserved_borrowed_type(&mut self) -> ParsedType {
        let span = self.cursor.current().span;
        self.cursor.advance();
        self.deferred_errors.push(ParseError::new(
            ErrorCode::E1001,
            "borrowed references (`&T`) are reserved for a future version of Ori",
            span,
        ));
        self.parse_type().unwrap_or(ParsedType::Infer)
    }

    fn consume_type_delimiter_if(&mut self, kind: &TokenKind) {
        if self.cursor.check(kind) {
            self.cursor.advance();
        }
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
        // The series error is intentionally discarded: this helper is invoked
        // speculatively to disambiguate generic args (`Foo<T>`) from a comparison
        // (`a < b`). A soft failure here is the disambiguation signal; the caller
        // recovers via the `Gt` fallthrough below. Surfacing it would leak a
        // speculative-path error into the real diagnostic list (per §SN-1).
        let _ = self.series_direct(&SeriesConfig::comma(TokenKind::Gt).no_newlines(), |p| {
            if p.cursor.check(&TokenKind::Gt) {
                return Ok(false);
            }
            let tag = p.cursor.current_tag();
            if matches!(
                tag,
                TK::TAG_DOLLAR | TK::TAG_INT | TK::TAG_MINUS | TK::TAG_TRUE | TK::TAG_FALSE
            ) {
                // Const expression in type argument position: $N, $N + 1, 42, -1, true
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
        let first = self.parse_type()?;
        let first_id = self.arena.alloc_parsed_type(first.clone());
        let mut element_ids = vec![first_id];

        // Collect remaining elements if tuple
        let mut saw_comma = false;
        while self.cursor.check(&TokenKind::Comma) {
            self.cursor.advance();
            saw_comma = true;
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

        // If single element without arrow, check if trailing comma was consumed.
        // If no comma was seen, it is a parenthesized type, not a tuple.
        if element_ids.len() == 1 && !saw_comma {
            return Some(first);
        }

        let elems = self.arena.alloc_parsed_type_list(element_ids);
        Some(ParsedType::tuple(elems))
    }
}

#[cfg(test)]
mod tests;
