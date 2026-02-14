//! Constant parsing.

use crate::recovery::TokenSet;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{ConstDef, DurationUnit, Expr, ExprKind, Name, SizeUnit, TokenKind, Visibility};

/// Tokens valid as constant literal values.
const CONST_LITERAL_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Int(0))
    .with(TokenKind::Float(0))
    .with(TokenKind::String(Name::EMPTY))
    .with(TokenKind::True)
    .with(TokenKind::False)
    .with(TokenKind::Char('\0'))
    .with(TokenKind::Duration(0, DurationUnit::Nanoseconds))
    .with(TokenKind::Size(0, SizeUnit::Bytes));

impl Parser<'_> {
    /// Parse a constant declaration.
    ///
    /// Grammar: `constant_decl = "let" "$" identifier [ ":" type ] "=" expression`
    /// Syntax: `[pub] let $name = literal` or `[pub] let $name: type = literal`
    ///
    /// Returns `EmptyErr` if no `$` is present.
    pub(crate) fn parse_const(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        if !self.cursor.check(&TokenKind::Dollar) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Dollar,
                self.cursor.current_span().start as usize,
            );
        }

        self.parse_const_body(visibility)
    }

    fn parse_const_body(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        let start_span = self.cursor.current_span();

        // $
        committed!(self.cursor.expect(&TokenKind::Dollar));

        // name
        let name = committed!(self.cursor.expect_ident());

        // Optional type annotation: `: type`
        let ty = if self.cursor.check(&TokenKind::Colon) {
            self.cursor.advance();
            self.parse_type()
        } else {
            None
        };

        // =
        committed!(self.cursor.expect(&TokenKind::Eq));

        // literal value
        let value = require!(self, self.parse_literal_expr(), "literal value");

        let span = start_span.merge(self.cursor.previous_span());

        ParseOutcome::consumed_ok(ConstDef {
            name,
            ty,
            value,
            span,
            visibility,
        })
    }

    /// Parse a literal expression for constant values.
    ///
    /// Returns `EmptyErr` if the current token is not a valid literal.
    fn parse_literal_expr(&mut self) -> ParseOutcome<ori_ir::ExprId> {
        let span = self.cursor.current_span();
        let kind = match *self.cursor.current_kind() {
            TokenKind::Int(n) => {
                self.cursor.advance();
                let Ok(value) = i64::try_from(n) else {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large".to_string(),
                            span,
                        ),
                        span,
                    );
                };
                ExprKind::Int(value)
            }
            TokenKind::Float(bits) => {
                self.cursor.advance();
                ExprKind::Float(bits)
            }
            TokenKind::String(s) => {
                self.cursor.advance();
                ExprKind::String(s)
            }
            TokenKind::True => {
                self.cursor.advance();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.cursor.advance();
                ExprKind::Bool(false)
            }
            TokenKind::Char(c) => {
                self.cursor.advance();
                ExprKind::Char(c)
            }
            // Duration literals (e.g., 100ms, 30s)
            TokenKind::Duration(value, unit) => {
                self.cursor.advance();
                ExprKind::Duration { value, unit }
            }
            // Size literals (e.g., 4kb, 10mb)
            TokenKind::Size(value, unit) => {
                self.cursor.advance();
                ExprKind::Size { value, unit }
            }
            _ => {
                return ParseOutcome::empty_err(
                    CONST_LITERAL_TOKENS,
                    self.cursor.current_span().start as usize,
                );
            }
        };

        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(kind, span)))
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::StringInterner;

    fn parse_module(source: &str) -> crate::ParseOutput {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let parser = crate::Parser::new(&tokens, &interner);
        parser.parse_module()
    }

    #[test]
    fn test_const_without_type() {
        // Regression guard: let $PI = 3.14 (no type annotation)
        let output = parse_module("let $PI = 3.14");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_none());
    }

    #[test]
    fn test_const_with_type_int() {
        // Typed constant: let $MAX_SIZE: int = 1000
        let output = parse_module("let $MAX_SIZE: int = 1000");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }

    #[test]
    fn test_const_with_type_str() {
        // Typed string constant: let $NAME: str = "ori"
        let output = parse_module(r#"let $NAME: str = "ori""#);
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }

    #[test]
    fn test_const_with_type_bool() {
        // Typed bool constant: let $DEBUG: bool = false
        let output = parse_module("let $DEBUG: bool = false");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }

    #[test]
    fn test_pub_const_with_type() {
        // Pub typed constant: pub let $MAX: int = 100
        let output = parse_module("pub let $MAX: int = 100");
        assert!(
            output.errors.is_empty(),
            "Parse errors: {:?}",
            output.errors
        );
        assert_eq!(output.module.consts.len(), 1);
        assert!(output.module.consts[0].ty.is_some());
    }
}
