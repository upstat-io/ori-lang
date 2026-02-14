//! Generic parameters, bounds, and where clause parsing.
//!
//! All functions in this module return `ParseOutcome<T>` with structural
//! soft/hard error distinction:
//! - `EmptyErr`: Entry token not found (e.g., no `<` for generics) — try alternatives
//! - `ConsumedErr`: Committed to parse path but failed — report error
//! - `ConsumedOk`: Successfully parsed after consuming tokens

use crate::{chain, committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    CapabilityRef, GenericParam, GenericParamRange, Name, ParsedType, ParsedTypeId,
    ParsedTypeRange, TokenKind, TraitBound, WhereClause,
};

impl Parser<'_> {
    /// Parse a required type annotation.
    ///
    /// Returns `ConsumedOk` with the parsed type, or `EmptyErr` if no type
    /// is present at the current position. Callers should use `require!` to
    /// upgrade the soft error when a type is mandatory.
    pub(crate) fn parse_type_required(&mut self) -> ParseOutcome<ParsedType> {
        if let Some(ty) = self.parse_type() {
            // parse_type() consumed tokens when returning Some
            ParseOutcome::consumed_ok(ty)
        } else {
            // No type found — soft error for callers to handle
            ParseOutcome::empty_err_expected(
                &TokenKind::Ident(Name::EMPTY),
                self.cursor.current_span().start as usize,
            )
        }
    }

    /// Parse generic parameters: `<T, U: Bound>`, `<T, U: Bound = DefaultType>`, or `<$N: int>`.
    ///
    /// Returns `EmptyErr` if no `<` is present (generics are optional).
    /// Returns `ConsumedErr` for malformed generics after `<` is consumed.
    ///
    /// Supports:
    /// - Type parameters: `T`, `T: Bound`, `T = DefaultType`
    /// - Const generics: `$N: int`, `$N: int = 10`
    /// - Default type parameters for traits: `trait Add<Rhs = Self>`
    pub(crate) fn parse_generics(&mut self) -> ParseOutcome<GenericParamRange> {
        use crate::series::SeriesConfig;

        if !self.cursor.check(&TokenKind::Lt) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Lt,
                self.cursor.current_span().start as usize,
            );
        }

        // Committed: `<` confirmed, all errors from here are hard errors
        committed!(self.cursor.expect(&TokenKind::Lt));

        let start = self.arena.start_generic_params();
        committed!(
            self.series_direct(&SeriesConfig::comma(TokenKind::Gt).no_newlines(), |p| {
                if p.cursor.check(&TokenKind::Gt) {
                    return Ok(false);
                }

                let param_span = p.cursor.current_span();

                // Check for const generic: $N
                let is_const = p.cursor.check(&TokenKind::Dollar);
                if is_const {
                    p.cursor.advance(); // consume $
                }

                let name = p.cursor.expect_ident()?;

                if is_const {
                    // Const generic: $N: int [= default]
                    // Type is required for const generics
                    p.cursor.expect(&TokenKind::Colon)?;
                    let const_type = Some(p.parse_type_required().into_result()?);

                    // Optional default value (expression, not type)
                    // Use parse_non_comparison_expr to avoid `>` being treated as comparison
                    let default_value = if p.cursor.check(&TokenKind::Eq) {
                        p.cursor.advance();
                        Some(p.parse_non_comparison_expr().into_result()?)
                    } else {
                        None
                    };

                    p.arena.push_generic_param(GenericParam {
                        name,
                        bounds: Vec::new(),
                        default_type: None,
                        is_const: true,
                        const_type,
                        default_value,
                        span: param_span.merge(p.cursor.previous_span()),
                    });
                } else {
                    // Type parameter: T [: Bounds] [= Default]
                    // Optional bounds: : Bound + OtherBound
                    let bounds = if p.cursor.check(&TokenKind::Colon) {
                        p.cursor.advance();
                        p.parse_bounds().into_result()?
                    } else {
                        Vec::new()
                    };

                    // Optional default type: = Type
                    let default_type = if p.cursor.check(&TokenKind::Eq) {
                        p.cursor.advance();
                        Some(p.parse_type_required().into_result()?)
                    } else {
                        None
                    };

                    p.arena.push_generic_param(GenericParam {
                        name,
                        bounds,
                        default_type,
                        is_const: false,
                        const_type: None,
                        default_value: None,
                        span: param_span.merge(p.cursor.previous_span()),
                    });
                }
                Ok(true)
            })
        );

        committed!(self.cursor.expect(&TokenKind::Gt));
        ParseOutcome::consumed_ok(self.arena.finish_generic_params(start))
    }

    /// Parse trait bounds: `Eq + Clone + Printable`
    ///
    /// Returns `EmptyErr` if no identifier is found (no first bound).
    /// After the first bound is parsed, subsequent bounds after `+` are mandatory.
    pub(crate) fn parse_bounds(&mut self) -> ParseOutcome<Vec<TraitBound>> {
        let mut bounds = Vec::new();

        // First bound — EmptyErr propagates if no identifier found
        let bound_span = self.cursor.current_span();
        let (first, rest) = chain!(self, self.parse_type_path_parts());
        bounds.push(TraitBound {
            first,
            rest,
            span: bound_span.merge(self.cursor.previous_span()),
        });

        // Additional bounds separated by `+`
        while self.cursor.check(&TokenKind::Plus) {
            self.cursor.advance();
            let bound_span = self.cursor.current_span();
            let (first, rest) =
                require!(self, self.parse_type_path_parts(), "trait bound after `+`");
            bounds.push(TraitBound {
                first,
                rest,
                span: bound_span.merge(self.cursor.previous_span()),
            });
        }

        ParseOutcome::consumed_ok(bounds)
    }

    /// Parse a type path: `Name` or `std.collections.List`
    ///
    /// Returns `EmptyErr` if no identifier is present.
    fn parse_type_path(&mut self) -> ParseOutcome<Vec<Name>> {
        let (first, rest) = chain!(self, self.parse_type_path_parts());
        let mut segments = vec![first];
        segments.extend(rest);
        ParseOutcome::consumed_ok(segments)
    }

    /// Parse a type path as (`first_segment`, `rest_segments`).
    ///
    /// Guarantees at least one segment by returning the first separately.
    /// Returns `EmptyErr` if no identifier is found at the current position.
    fn parse_type_path_parts(&mut self) -> ParseOutcome<(Name, Vec<Name>)> {
        let Ok(first) = self.cursor.expect_ident() else {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Ident(Name::EMPTY),
                self.cursor.current_span().start as usize,
            );
        };

        let mut rest = Vec::new();
        while self.cursor.check(&TokenKind::Dot) {
            self.cursor.advance();
            // After `.`, an identifier is mandatory
            let segment = committed!(self.cursor.expect_ident());
            rest.push(segment);
        }

        ParseOutcome::consumed_ok((first, rest))
    }

    /// Parse a type for impl blocks: `Name` or `Name<T, U>`.
    ///
    /// Returns `EmptyErr` if no identifier is found.
    /// Returns (path, `ParsedType`) where path is the type name(s) for registration.
    pub(crate) fn parse_impl_type(&mut self) -> ParseOutcome<(Vec<Name>, ParsedType)> {
        let path = chain!(self, self.parse_type_path());
        let name = *committed!(path.last().ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "empty type path".to_string(),
                self.cursor.current_span(),
            )
        }));

        // Check for type arguments: <T, U>
        let type_args = if self.cursor.check(&TokenKind::Lt) {
            use crate::series::SeriesConfig;

            self.cursor.advance(); // <
                                   // Type arg lists use a Vec because nested generic args share the
                                   // same `parsed_type_lists` buffer (e.g., `Impl<Option<T>>`).
            let mut type_args: Vec<ParsedTypeId> = Vec::new();
            committed!(self.series_direct(
                &SeriesConfig::comma(TokenKind::Gt).no_newlines(),
                |p| {
                    if p.cursor.check(&TokenKind::Gt) {
                        return Ok(false);
                    }
                    let ty = p.parse_type_required().into_result()?;
                    type_args.push(p.arena.alloc_parsed_type(ty));
                    Ok(true)
                }
            ));
            if self.cursor.check(&TokenKind::Gt) {
                self.cursor.advance(); // >
            }
            self.arena.alloc_parsed_type_list(type_args)
        } else {
            ParsedTypeRange::EMPTY
        };

        let ty = ParsedType::Named { name, type_args };
        ParseOutcome::consumed_ok((path, ty))
    }

    /// Parse uses clause: `uses Http, FileSystem, Async`
    ///
    /// Returns `EmptyErr` if no `uses` keyword is present.
    pub(crate) fn parse_uses_clause(&mut self) -> ParseOutcome<Vec<CapabilityRef>> {
        if !self.cursor.check(&TokenKind::Uses) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Uses,
                self.cursor.current_span().start as usize,
            );
        }

        // Committed: `uses` keyword confirmed
        committed!(self.cursor.expect(&TokenKind::Uses));

        let mut capabilities = Vec::new();
        loop {
            let cap_span = self.cursor.current_span();
            let name = committed!(self.cursor.expect_ident());

            capabilities.push(CapabilityRef {
                name,
                span: cap_span,
            });

            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else {
                break;
            }
        }

        ParseOutcome::consumed_ok(capabilities)
    }

    /// Parse where clauses: `where T: Clone, U: Default, T.Item: Eq, N > 0`
    ///
    /// Returns `EmptyErr` if no `where` keyword is present.
    ///
    /// Supports two kinds of clauses:
    /// - Type bounds: `T: Clone`, `T.Item: Eq`
    /// - Const bounds: `N > 0`, `N + M == 10`
    ///
    /// Disambiguation: if we see `ident :` or `ident . ident :`, it's a type bound.
    /// Otherwise, the entire clause is parsed as a const expression.
    pub(crate) fn parse_where_clauses(&mut self) -> ParseOutcome<Vec<WhereClause>> {
        if !self.cursor.check(&TokenKind::Where) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Where,
                self.cursor.current_span().start as usize,
            );
        }

        // Committed: `where` keyword confirmed
        committed!(self.cursor.expect(&TokenKind::Where));

        let mut clauses = Vec::new();
        loop {
            let clause_span = self.cursor.current_span();

            // Disambiguation: check if this looks like a type bound (ident + : or ident.ident + :)
            let is_type_bound = self.cursor.check_ident() && {
                let next = self.cursor.peek_next_kind();
                matches!(next, TokenKind::Colon | TokenKind::Dot)
            };

            if is_type_bound {
                let param = committed!(self.cursor.expect_ident());

                // Check for associated type projection: T.Item
                let projection = if self.cursor.check(&TokenKind::Dot) {
                    self.cursor.advance();
                    Some(committed!(self.cursor.expect_ident()))
                } else {
                    None
                };

                committed!(self.cursor.expect(&TokenKind::Colon));
                let bounds = require!(self, self.parse_bounds(), "trait bounds in where clause");

                clauses.push(WhereClause::TypeBound {
                    param,
                    projection,
                    bounds,
                    span: clause_span.merge(self.cursor.previous_span()),
                });
            } else {
                // Const bound: parse as expression without assignment (N > 0, N + M == 10).
                // Use parse_non_assign_expr because `=` delimits the function body.
                let expr = committed!(self.parse_non_assign_expr().into_result());
                clauses.push(WhereClause::ConstBound {
                    expr,
                    span: clause_span.merge(self.cursor.previous_span()),
                });
            }

            if self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
            } else {
                break;
            }
        }

        ParseOutcome::consumed_ok(clauses)
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::{StringInterner, WhereClause};

    /// Parse a source string and return the where clauses from the first function.
    fn parse_where_clauses(source: &str) -> Vec<WhereClause> {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let parser = crate::Parser::new(&tokens, &interner);
        let output = parser.parse_module();
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        output.module.functions[0].where_clauses.clone()
    }

    #[test]
    fn test_where_type_bound() {
        // Regression: T: Clone still parses as TypeBound
        let clauses = parse_where_clauses("@f<T> () -> void where T: Clone = ()");
        assert_eq!(clauses.len(), 1);
        assert!(clauses[0].is_type_bound());
    }

    #[test]
    fn test_where_type_bound_with_projection() {
        // T.Item: Eq — associated type constraint
        let clauses = parse_where_clauses("@f<T> () -> void where T.Item: Eq = ()");
        assert_eq!(clauses.len(), 1);
        assert!(clauses[0].is_type_bound());
    }

    #[test]
    fn test_where_multiple_type_bounds() {
        // Multiple type bounds: T: Clone, U: Default
        let clauses = parse_where_clauses("@f<T, U> () -> void where T: Clone, U: Default = ()");
        assert_eq!(clauses.len(), 2);
        assert!(clauses[0].is_type_bound());
        assert!(clauses[1].is_type_bound());
    }

    #[test]
    fn test_where_const_bound() {
        // N > 0 — const bound expression
        let clauses = parse_where_clauses("@f<$N: int> () -> void where N > 0 = ()");
        assert_eq!(clauses.len(), 1);
        assert!(clauses[0].is_const_bound());
    }

    #[test]
    fn test_where_mixed_type_and_const_bounds() {
        // T: Clone, N > 0 — mixed type bound + const bound
        let clauses = parse_where_clauses("@f<T, $N: int> () -> void where T: Clone, N > 0 = ()");
        assert_eq!(clauses.len(), 2);
        assert!(clauses[0].is_type_bound());
        assert!(clauses[1].is_const_bound());
    }
}
