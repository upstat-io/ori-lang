//! Let binding and binding pattern parsing.
//!
//! Handles `let` expressions and destructuring binding patterns
//! (name, wildcard, tuple, struct, list).

use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    BindingPattern, Expr, ExprId, ExprKind, FieldBinding, Mutability, ParsedTypeId, TokenKind,
};

impl Parser<'_> {
    /// Parse let expression.
    ///
    /// Per spec (05-variables.md): Bindings are mutable by default.
    /// - `let x = ...` → mutable (default)
    /// - `let $x = ...` → immutable ($ prefix)
    ///
    /// Guard: returns `EmptyErr` if not at `let`.
    pub(super) fn parse_let_expr(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::Let) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Let,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(crate::ErrorContext::LetPattern, Self::parse_let_expr_body)
    }

    fn parse_let_expr_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        self.cursor.advance();

        // Don't consume `$` here — let parse_binding_pattern() handle it
        // so that BindingPattern::Name.mutable is set correctly for both
        // simple bindings (`let $x = 5`) and destructuring (`let ($a, b) = ...`).
        let pattern = committed!(self.parse_binding_pattern());

        // Derive expression-level mutability from the pattern.
        // For simple Name patterns, this comes from the `$` prefix.
        // For compound patterns (tuple, struct, list), default to mutable
        // since per-binding mutability is tracked on sub-patterns.
        let mutable = match &pattern {
            BindingPattern::Name { mutable, .. } => *mutable,
            _ => Mutability::Mutable,
        };
        let pattern_id = self.arena.alloc_binding_pattern(pattern);

        let ty = if self.cursor.check(&TokenKind::Colon) {
            self.cursor.advance();
            self.parse_type()
                .map_or(ParsedTypeId::INVALID, |t| self.arena.alloc_parsed_type(t))
        } else {
            ParsedTypeId::INVALID
        };

        committed!(self.cursor.expect(&TokenKind::Eq));
        let init = require!(self, self.parse_expr(), "initializer expression");

        let end_span = self.arena.get_expr(init).span;
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Let {
                pattern: pattern_id,
                ty,
                init,
                mutable,
            },
            span.merge(end_span),
        )))
    }

    /// Parse a binding pattern.
    ///
    /// Per grammar: `binding_pattern = [ "$" ] identifier | "_" | "{" ... "}" | ...`
    /// The `$` prefix marks the binding as immutable.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive binding pattern dispatch across destructuring, name, and wildcard forms"
    )]
    pub(crate) fn parse_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        // Handle $ prefix for immutable bindings: $x, $name, etc.
        if self.cursor.check(&TokenKind::Dollar) {
            self.cursor.advance();
            if let Some(name_str) = self.cursor.soft_keyword_to_name() {
                let name = self.cursor.interner().intern(name_str);
                self.cursor.advance();
                return Ok(BindingPattern::Name {
                    name,
                    mutable: Mutability::Immutable,
                });
            }
            if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                self.cursor.advance();
                return Ok(BindingPattern::Name {
                    name,
                    mutable: Mutability::Immutable,
                });
            }
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!(
                    "expected identifier after $, found {}",
                    self.cursor.current_kind().display_name()
                ),
                self.cursor.current_span(),
            ));
        }

        if let Some(name_str) = self.cursor.soft_keyword_to_name() {
            let name = self.cursor.interner().intern(name_str);
            self.cursor.advance();
            return Ok(BindingPattern::Name {
                name,
                mutable: Mutability::Mutable,
            });
        }

        match *self.cursor.current_kind() {
            TokenKind::Ident(name) => {
                self.cursor.advance();
                Ok(BindingPattern::Name {
                    name,
                    mutable: Mutability::Mutable,
                })
            }
            TokenKind::Underscore => {
                self.cursor.advance();
                Ok(BindingPattern::Wildcard)
            }
            TokenKind::LParen => {
                use crate::series::SeriesConfig;
                self.cursor.advance();
                let patterns: Vec<BindingPattern> =
                    self.series(&SeriesConfig::comma(TokenKind::RParen).no_newlines(), |p| {
                        if p.cursor.check(&TokenKind::RParen) {
                            Ok(None)
                        } else {
                            Ok(Some(p.parse_binding_pattern()?))
                        }
                    })?;
                self.cursor.expect(&TokenKind::RParen)?;
                Ok(BindingPattern::Tuple(patterns))
            }
            TokenKind::LBrace => {
                use crate::series::SeriesConfig;
                self.cursor.advance();
                let fields: Vec<FieldBinding> =
                    self.series(&SeriesConfig::comma(TokenKind::RBrace).no_newlines(), |p| {
                        if p.cursor.check(&TokenKind::RBrace) {
                            return Ok(None);
                        }

                        // Per grammar: field_binding = [ "$" ] identifier [ ":" binding_pattern ]
                        let mutable = if p.cursor.check(&TokenKind::Dollar) {
                            p.cursor.advance();
                            Mutability::Immutable
                        } else {
                            Mutability::Mutable
                        };

                        let field_name = p.cursor.expect_ident()?;

                        let pattern = if p.cursor.check(&TokenKind::Colon) {
                            p.cursor.advance();
                            Some(p.parse_binding_pattern()?)
                        } else {
                            None // Shorthand: { x } binds field x to variable x
                        };

                        Ok(Some(FieldBinding {
                            name: field_name,
                            mutable,
                            pattern,
                        }))
                    })?;
                self.cursor.expect(&TokenKind::RBrace)?;
                Ok(BindingPattern::Struct { fields })
            }
            TokenKind::LBracket => {
                // List pattern is special: has optional ..rest at the end
                // Cannot use simple series combinator
                self.cursor.advance();
                let mut elements = Vec::new();
                let mut rest = None;

                while !self.cursor.check(&TokenKind::RBracket) && !self.cursor.is_at_end() {
                    if self.cursor.check(&TokenKind::DotDot) {
                        self.cursor.advance();
                        // Check for optional `$` (immutable) prefix on rest binding
                        let rest_mutable = if self.cursor.check(&TokenKind::Dollar) {
                            self.cursor.advance();
                            Mutability::Immutable
                        } else {
                            Mutability::Mutable
                        };
                        if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                            rest = Some((name, rest_mutable));
                            self.cursor.advance();
                        }
                        break;
                    }
                    elements.push(self.parse_binding_pattern()?);
                    if !self.cursor.check(&TokenKind::RBracket)
                        && !self.cursor.check(&TokenKind::DotDot)
                    {
                        self.cursor.expect(&TokenKind::Comma)?;
                    }
                }
                self.cursor.expect(&TokenKind::RBracket)?;
                Ok(BindingPattern::List { elements, rest })
            }
            _ => Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!(
                    "expected binding pattern, found {}",
                    self.cursor.current_kind().display_name()
                ),
                self.cursor.current_span(),
            )),
        }
    }
}
