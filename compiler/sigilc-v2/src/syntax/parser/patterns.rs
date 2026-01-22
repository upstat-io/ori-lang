//! Match pattern parsing.

use crate::errors::Diagnostic;
use crate::syntax::{
    TokenKind,
    Expr, ExprKind, ExprId,
    expr::{MatchArm, MatchPattern},
};
use super::Parser;

impl<'src, 'i> Parser<'src, 'i> {
    /// Parse a match expression.
    pub(crate) fn parse_match_expr(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::Match, "expected 'match'")?;
        self.consume(&TokenKind::LParen, "expected '('")?;
        self.skip_newlines();

        // Parse scrutinee (the value being matched)
        let scrutinee = self.expression()?;

        self.consume(&TokenKind::Comma, "expected ','")?;
        self.skip_newlines();

        // Parse match arms: pattern -> body
        let mut arms = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.at_end() {
            let arm_start = self.current_span();

            let pattern = self.parse_match_pattern()?;

            // Check for guard: pattern.match(guard_expr) -> body
            let guard = if self.check(&TokenKind::Dot) {
                let next_pos = self.pos + 1;
                if next_pos < self.tokens.tokens.len() {
                    if let TokenKind::Match = self.tokens.tokens[next_pos].kind {
                        self.advance(); // consume .
                        self.advance(); // consume match
                        self.consume(&TokenKind::LParen, "expected '('")?;
                        let guard_expr = self.expression()?;
                        self.consume(&TokenKind::RParen, "expected ')'")?;
                        Some(guard_expr)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            self.consume(&TokenKind::Arrow, "expected '->'")?;
            self.skip_newlines();

            let body = self.expression()?;
            let arm_end = self.arena.get(body).span;

            arms.push(MatchArm {
                pattern,
                guard,
                body,
                span: arm_start.merge(arm_end),
            });

            if self.check(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            }
        }

        self.consume(&TokenKind::RParen, "expected ')'")?;

        let arms_range = self.arena.alloc_arms(arms);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Match { scrutinee, arms: arms_range },
            start.merge(self.current_span()),
        )))
    }

    pub(crate) fn parse_match_pattern(&mut self) -> Result<MatchPattern, Diagnostic> {
        let span = self.current_span();

        match self.current_kind() {
            // Wildcard: _
            TokenKind::Underscore => {
                self.advance();
                Ok(MatchPattern::Wildcard)
            }

            // Literal patterns
            TokenKind::Int(n) => {
                let n = *n;
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Int(n), span));
                self.parse_pattern_suffix(MatchPattern::Literal(expr))
            }
            TokenKind::Float(bits) => {
                let bits = *bits;
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Float(f64::from_bits(bits)), span));
                self.parse_pattern_suffix(MatchPattern::Literal(expr))
            }
            TokenKind::String(s) => {
                let s = *s;
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::String(s), span));
                self.parse_pattern_suffix(MatchPattern::Literal(expr))
            }
            TokenKind::True => {
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Bool(true), span));
                self.parse_pattern_suffix(MatchPattern::Literal(expr))
            }
            TokenKind::False => {
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Bool(false), span));
                self.parse_pattern_suffix(MatchPattern::Literal(expr))
            }

            // Variant patterns: Some(x), None, Ok(x), Err(e)
            TokenKind::Some => {
                self.advance();
                self.consume(&TokenKind::LParen, "expected '('")?;
                let inner = self.parse_match_pattern()?;
                self.consume(&TokenKind::RParen, "expected ')'")?;
                Ok(MatchPattern::Variant {
                    name: self.interner.intern("Some"),
                    inner: Some(Box::new(inner)),
                })
            }
            TokenKind::None => {
                self.advance();
                Ok(MatchPattern::Variant {
                    name: self.interner.intern("None"),
                    inner: None,
                })
            }
            TokenKind::Ok => {
                self.advance();
                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let inner = self.parse_match_pattern()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    Ok(MatchPattern::Variant {
                        name: self.interner.intern("Ok"),
                        inner: Some(Box::new(inner)),
                    })
                } else {
                    Ok(MatchPattern::Variant {
                        name: self.interner.intern("Ok"),
                        inner: None,
                    })
                }
            }
            TokenKind::Err => {
                self.advance();
                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let inner = self.parse_match_pattern()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    Ok(MatchPattern::Variant {
                        name: self.interner.intern("Err"),
                        inner: Some(Box::new(inner)),
                    })
                } else {
                    Ok(MatchPattern::Variant {
                        name: self.interner.intern("Err"),
                        inner: None,
                    })
                }
            }

            // List pattern: []
            TokenKind::LBracket => {
                self.advance();
                self.skip_newlines();
                let mut elements = Vec::new();
                let mut rest = None;

                while !self.check(&TokenKind::RBracket) && !self.at_end() {
                    if self.check(&TokenKind::DotDot) {
                        self.advance();
                        if let TokenKind::Ident(name) = self.current_kind() {
                            rest = Some(*name);
                            self.advance();
                        }
                        break;
                    }

                    let elem = self.parse_match_pattern()?;
                    elements.push(elem);

                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                    self.skip_newlines();
                }

                self.consume(&TokenKind::RBracket, "expected ']'")?;
                Ok(MatchPattern::List { elements, rest })
            }

            // Binding or variant pattern: identifier
            TokenKind::Ident(name) => {
                let name = *name;
                self.advance();

                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let inner = self.parse_match_pattern()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    Ok(MatchPattern::Variant {
                        name,
                        inner: Some(Box::new(inner)),
                    })
                } else if self.check(&TokenKind::Pipe) {
                    let mut patterns = vec![MatchPattern::Binding(name)];
                    while self.check(&TokenKind::Pipe) {
                        self.advance();
                        self.skip_newlines();
                        patterns.push(self.parse_match_pattern()?);
                    }
                    Ok(MatchPattern::Or(patterns))
                } else {
                    Ok(MatchPattern::Binding(name))
                }
            }

            // Or pattern starting with literal
            _ if self.is_literal() => {
                let first = self.parse_match_pattern()?;
                if self.check(&TokenKind::Pipe) {
                    let mut patterns = vec![first];
                    while self.check(&TokenKind::Pipe) {
                        self.advance();
                        self.skip_newlines();
                        patterns.push(self.parse_match_pattern()?);
                    }
                    Ok(MatchPattern::Or(patterns))
                } else {
                    Ok(first)
                }
            }

            _ => Err(self.error("expected match pattern")),
        }
    }

    /// Parse pattern suffix: or-pattern (|) or range-pattern (..)
    fn parse_pattern_suffix(&mut self, first: MatchPattern) -> Result<MatchPattern, Diagnostic> {
        // Check for or-pattern: 1 | 2 | 3
        if self.check(&TokenKind::Pipe) {
            let mut patterns = vec![first];
            while self.check(&TokenKind::Pipe) {
                self.advance();
                self.skip_newlines();
                let next = self.parse_simple_match_pattern()?;
                patterns.push(next);
            }
            return Ok(MatchPattern::Or(patterns));
        }

        // Check for range-pattern: 0..10 or 0..=10
        if self.check(&TokenKind::DotDot) || self.check(&TokenKind::DotDotEq) {
            let inclusive = self.check(&TokenKind::DotDotEq);
            self.advance();

            let start_expr = match first {
                MatchPattern::Literal(expr) => Some(expr),
                _ => return Err(self.error("range pattern requires literal start")),
            };

            let end_expr = if self.is_literal() {
                let span = self.current_span();
                match self.current_kind() {
                    TokenKind::Int(n) => {
                        let n = *n;
                        self.advance();
                        Some(self.arena.alloc(Expr::new(ExprKind::Int(n), span)))
                    }
                    _ => None,
                }
            } else {
                None
            };

            return Ok(MatchPattern::Range {
                start: start_expr,
                end: end_expr,
                inclusive,
            });
        }

        Ok(first)
    }

    /// Parse a simple match pattern without suffix handling.
    fn parse_simple_match_pattern(&mut self) -> Result<MatchPattern, Diagnostic> {
        let span = self.current_span();
        match self.current_kind() {
            TokenKind::Underscore => {
                self.advance();
                Ok(MatchPattern::Wildcard)
            }
            TokenKind::Int(n) => {
                let n = *n;
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Int(n), span));
                Ok(MatchPattern::Literal(expr))
            }
            TokenKind::Float(bits) => {
                let bits = *bits;
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Float(f64::from_bits(bits)), span));
                Ok(MatchPattern::Literal(expr))
            }
            TokenKind::String(s) => {
                let s = *s;
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::String(s), span));
                Ok(MatchPattern::Literal(expr))
            }
            TokenKind::True => {
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Bool(true), span));
                Ok(MatchPattern::Literal(expr))
            }
            TokenKind::False => {
                self.advance();
                let expr = self.arena.alloc(Expr::new(ExprKind::Bool(false), span));
                Ok(MatchPattern::Literal(expr))
            }
            TokenKind::Ident(name) => {
                let name = *name;
                self.advance();
                Ok(MatchPattern::Binding(name))
            }
            _ => Err(self.error("expected pattern")),
        }
    }

    pub(crate) fn is_literal(&self) -> bool {
        matches!(
            self.current_kind(),
            TokenKind::Int(_) | TokenKind::Float(_) | TokenKind::String(_) |
            TokenKind::True | TokenKind::False | TokenKind::Char(_)
        )
    }
}
