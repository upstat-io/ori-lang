// Expression parsing entry point for Sigil
// Delegates to submodules for different expression types:
// - operators.rs: Operator precedence chain
// - postfix.rs: Field access, indexing, calls
// - primary.rs: Literals, keywords, control flow

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;

impl Parser {
    /// Parse an expression, returning it with its source span
    pub(super) fn parse_expr(&mut self) -> Result<SpannedExpr, String> {
        self.parse_or_expr()
    }

    /// Parse arguments to a function/method call, returning spanned expressions
    pub(super) fn parse_args(&mut self) -> Result<Vec<SpannedExpr>, String> {
        let mut args = Vec::new();

        self.skip_newlines();
        while !matches!(self.current(), Some(Token::RParen)) {
            args.push(self.parse_expr()?);
            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }

        Ok(args)
    }
}
