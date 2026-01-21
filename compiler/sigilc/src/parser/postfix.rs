// Postfix expression parsing for Sigil
// Handles field access, indexing, method calls, function calls, and coalesce

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;

impl Parser {
    pub(super) fn parse_postfix_expr(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            match self.current() {
                Some(Token::Dot) => {
                    self.advance();
                    let name = match self.current() {
                        Some(Token::Ident(n)) => {
                            let n = n.clone();
                            self.advance();
                            n
                        }
                        _ => return Err("Expected identifier after .".to_string()),
                    };

                    // Check for method call
                    if matches!(self.current(), Some(Token::LParen)) {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect(Token::RParen)?;
                        expr = Expr::MethodCall {
                            receiver: Box::new(expr),
                            method: name,
                            args,
                        };
                    } else {
                        expr = Expr::Field(Box::new(expr), name);
                    }
                }
                Some(Token::LBracket) => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
                }
                Some(Token::LParen) => {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    expr = Expr::Call {
                        func: Box::new(expr),
                        args,
                    };
                }
                Some(Token::DoubleQuestion) => {
                    self.advance();
                    let default = self.parse_unary_expr()?;
                    expr = Expr::Coalesce {
                        value: Box::new(expr),
                        default: Box::new(default),
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }
}
