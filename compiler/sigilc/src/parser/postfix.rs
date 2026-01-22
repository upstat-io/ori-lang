// Postfix expression parsing for Sigil
// Handles field access, indexing, method calls, function calls, and coalesce

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;

impl Parser {
    pub(super) fn parse_postfix_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
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
                        let new_expr = Expr::MethodCall {
                            receiver: Box::new(expr.expr),
                            method: name,
                            args: args.into_iter().map(|a| a.expr).collect(),
                        };
                        expr = self.spanned(new_expr, start);
                    } else {
                        let new_expr = Expr::Field(Box::new(expr.expr), name);
                        expr = self.spanned(new_expr, start);
                    }
                }
                Some(Token::LBracket) => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBracket)?;
                    let new_expr = Expr::Index(Box::new(expr.expr), Box::new(index.expr));
                    expr = self.spanned(new_expr, start);
                }
                Some(Token::LParen) => {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    let new_expr = Expr::Call {
                        func: Box::new(expr.expr),
                        args: args.into_iter().map(|a| a.expr).collect(),
                    };
                    expr = self.spanned(new_expr, start);
                }
                Some(Token::DoubleQuestion) => {
                    self.advance();
                    let default = self.parse_unary_expr()?;
                    let new_expr = Expr::Coalesce {
                        value: Box::new(expr.expr),
                        default: Box::new(default.expr),
                    };
                    expr = self.spanned(new_expr, start);
                }
                _ => break,
            }
        }

        Ok(expr)
    }
}
