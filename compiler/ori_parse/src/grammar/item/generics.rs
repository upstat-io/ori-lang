//! Generic parameters, bounds, and where clause parsing.

use crate::{ParseError, Parser};
use ori_ir::{
    CapabilityRef, GenericParam, GenericParamRange, Name, ParsedType, ParsedTypeRange, TokenKind,
    TraitBound, WhereClause,
};

impl Parser<'_> {
    /// Parse a type, accepting all type forms (primitives, named, compounds).
    /// Returns `ParsedType` representing the full type structure.
    pub(crate) fn parse_type_required(&mut self) -> Result<ParsedType, ParseError> {
        if let Some(ty) = self.parse_type() {
            return Ok(ty);
        }

        Err(ParseError::new(
            ori_diagnostic::ErrorCode::E1002,
            format!(
                "expected type, found {}",
                self.current_kind().display_name()
            ),
            self.current_span(),
        ))
    }

    /// Parse generic parameters: `<T, U: Bound>` or `<T, U: Bound = DefaultType>`
    ///
    /// Supports default type parameters for traits: `trait Add<Rhs = Self>`.
    pub(crate) fn parse_generics(&mut self) -> Result<GenericParamRange, ParseError> {
        self.expect(&TokenKind::Lt)?;

        let mut params = Vec::new();
        while !self.check(&TokenKind::Gt) && !self.is_at_end() {
            let param_span = self.current_span();
            let name = self.expect_ident()?;

            // Optional bounds: : Bound + OtherBound
            let bounds = if self.check(&TokenKind::Colon) {
                self.advance();
                self.parse_bounds()?
            } else {
                Vec::new()
            };

            // Optional default type: = Type
            let default_type = if self.check(&TokenKind::Eq) {
                self.advance();
                Some(self.parse_type_required()?)
            } else {
                None
            };

            params.push(GenericParam {
                name,
                bounds,
                default_type,
                span: param_span.merge(self.previous_span()),
            });

            if !self.check(&TokenKind::Gt) {
                self.expect(&TokenKind::Comma)?;
            }
        }

        self.expect(&TokenKind::Gt)?;
        Ok(self.arena.alloc_generic_params(params))
    }

    /// Parse trait bounds: Eq + Clone + Printable
    pub(crate) fn parse_bounds(&mut self) -> Result<Vec<TraitBound>, ParseError> {
        let mut bounds = Vec::new();

        loop {
            let bound_span = self.current_span();
            let (first, rest) = self.parse_type_path_parts()?;

            bounds.push(TraitBound {
                first,
                rest,
                span: bound_span.merge(self.previous_span()),
            });

            if self.check(&TokenKind::Plus) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(bounds)
    }

    /// Parse a type path: Name or std.collections.List
    fn parse_type_path(&mut self) -> Result<Vec<Name>, ParseError> {
        let (first, rest) = self.parse_type_path_parts()?;
        let mut segments = vec![first];
        segments.extend(rest);
        Ok(segments)
    }

    /// Parse a type path as (`first_segment`, `rest_segments`).
    /// Guarantees at least one segment by returning the first separately.
    fn parse_type_path_parts(&mut self) -> Result<(Name, Vec<Name>), ParseError> {
        let first = self.expect_ident()?;
        let mut rest = Vec::new();

        while self.check(&TokenKind::Dot) {
            self.advance();
            let segment = self.expect_ident()?;
            rest.push(segment);
        }

        Ok((first, rest))
    }

    /// Parse a type for impl blocks: `Name` or `Name<T, U>`.
    ///
    /// Returns (path, `ParsedType`) where path is the type name(s) for registration.
    pub(crate) fn parse_impl_type(&mut self) -> Result<(Vec<Name>, ParsedType), ParseError> {
        let path = self.parse_type_path()?;
        let name = *path.last().ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "empty type path".to_string(),
                self.current_span(),
            )
        })?;

        // Check for type arguments: <T, U>
        let type_args = if self.check(&TokenKind::Lt) {
            self.advance(); // <
            let mut arg_ids = Vec::new();
            while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                let ty = self.parse_type_required()?;
                let id = self.arena.alloc_parsed_type(ty);
                arg_ids.push(id);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            if self.check(&TokenKind::Gt) {
                self.advance(); // >
            }
            self.arena.alloc_parsed_type_list(arg_ids)
        } else {
            ParsedTypeRange::EMPTY
        };

        let ty = ParsedType::Named { name, type_args };
        Ok((path, ty))
    }

    /// Parse uses clause: uses Http, `FileSystem`, Async
    pub(crate) fn parse_uses_clause(&mut self) -> Result<Vec<CapabilityRef>, ParseError> {
        self.expect(&TokenKind::Uses)?;

        let mut capabilities = Vec::new();
        loop {
            let cap_span = self.current_span();
            let name = self.expect_ident()?;

            capabilities.push(CapabilityRef {
                name,
                span: cap_span,
            });

            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(capabilities)
    }

    /// Parse where clauses: where T: Clone, U: Default, T.Item: Eq
    pub(crate) fn parse_where_clauses(&mut self) -> Result<Vec<WhereClause>, ParseError> {
        self.expect(&TokenKind::Where)?;

        let mut clauses = Vec::new();
        loop {
            let clause_span = self.current_span();
            let param = self.expect_ident()?;

            // Check for associated type projection: T.Item
            let projection = if self.check(&TokenKind::Dot) {
                self.advance();
                Some(self.expect_ident()?)
            } else {
                None
            };

            self.expect(&TokenKind::Colon)?;
            let bounds = self.parse_bounds()?;

            clauses.push(WhereClause {
                param,
                projection,
                bounds,
                span: clause_span.merge(self.previous_span()),
            });

            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(clauses)
    }
}
