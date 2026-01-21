// Primary expression parsing for Sigil
// Handles literals, keywords, identifiers, control flow, lambdas, and structural expressions

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;

impl Parser {
    pub(super) fn parse_primary_expr(&mut self) -> Result<Expr, String> {
        match self.current() {
            Some(Token::Int(n)) => {
                let n = *n;
                self.advance();
                Ok(Expr::Int(n))
            }
            Some(Token::Float(f)) => {
                let f = *f;
                self.advance();
                Ok(Expr::Float(f))
            }
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::String(s))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Some(Token::Nil) => {
                self.advance();
                Ok(Expr::Nil)
            }
            Some(Token::Ok_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Ok(Box::new(value)))
            }
            Some(Token::Err_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Err(Box::new(value)))
            }
            Some(Token::Some_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Some(Box::new(value)))
            }
            Some(Token::None_) => {
                self.advance();
                Ok(Expr::None_)
            }
            Some(Token::Assert) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("assert".to_string())),
                    args,
                })
            }
            Some(Token::AssertErr) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("assert_err".to_string())),
                    args,
                })
            }
            // Type keywords used as conversion functions: str(), int(), etc.
            Some(Token::StrType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("str".to_string())),
                    args,
                })
            }
            Some(Token::IntType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("int".to_string())),
                    args,
                })
            }
            Some(Token::FloatType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("float".to_string())),
                    args,
                })
            }
            Some(Token::BoolType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("bool".to_string())),
                    args,
                })
            }
            Some(Token::Dollar) => {
                self.advance();
                match self.current() {
                    Some(Token::Ident(n)) => {
                        let n = n.clone();
                        self.advance();
                        Ok(Expr::Config(n))
                    }
                    _ => Err("Expected identifier after $".to_string()),
                }
            }
            Some(Token::Hash) => {
                // # is length placeholder (arr[# - 1] means arr[length - 1])
                self.advance();
                Ok(Expr::LengthPlaceholder)
            }
            Some(Token::Let) => {
                self.advance();
                let mutable = if matches!(self.current(), Some(Token::Mut)) {
                    self.advance();
                    true
                } else {
                    false
                };
                let name = match self.current() {
                    Some(Token::Ident(n)) => {
                        let n = n.clone();
                        self.advance();
                        n
                    }
                    _ => return Err("Expected identifier after 'let'".to_string()),
                };
                self.expect(Token::Eq)?;
                let value = self.parse_expr()?;
                Ok(Expr::Let {
                    name,
                    mutable,
                    value: Box::new(value),
                })
            }
            Some(Token::Match) => {
                self.advance();
                self.parse_match_expr()
            }
            Some(Token::LParen) => self.parse_paren_expr(),
            Some(Token::LBracket) => self.parse_list_literal(),
            Some(Token::If) => self.parse_if_expr(),
            Some(Token::For) => self.parse_for_expr(),
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                self.parse_ident_continuation(n)
            }
            // Standalone operators as values (for fold, etc.)
            Some(Token::Plus) => {
                self.advance();
                Ok(Expr::Ident("+".to_string()))
            }
            Some(Token::Star) => {
                self.advance();
                Ok(Expr::Ident("*".to_string()))
            }
            Some(Token::Minus) => self.parse_minus_expr(),
            _ => Err(format!(
                "Unexpected token in expression: {:?}",
                self.current()
            )),
        }
    }

    fn parse_paren_expr(&mut self) -> Result<Expr, String> {
        self.advance(); // consume '('
        if matches!(self.current(), Some(Token::RParen)) {
            self.advance();
            // Check for lambda with no params: () -> expr
            if matches!(self.current(), Some(Token::Arrow)) {
                self.advance();
                let body = self.parse_expr()?;
                return Ok(Expr::Lambda {
                    params: Vec::new(),
                    body: Box::new(body),
                });
            }
            return Ok(Expr::Tuple(Vec::new()));
        }
        let expr = self.parse_expr()?;
        if matches!(self.current(), Some(Token::Comma)) {
            // Could be tuple or multi-param lambda
            let mut exprs = vec![expr];
            while matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                if matches!(self.current(), Some(Token::RParen)) {
                    break;
                }
                exprs.push(self.parse_expr()?);
            }
            self.expect(Token::RParen)?;
            // Check for multi-param lambda: (a, b) -> expr
            if matches!(self.current(), Some(Token::Arrow)) {
                self.advance();
                // Convert exprs to param names
                let params: Result<Vec<String>, String> = exprs
                    .into_iter()
                    .map(|e| match e {
                        Expr::Ident(n) => Ok(n),
                        _ => Err("Lambda parameters must be identifiers".to_string()),
                    })
                    .collect();
                let body = self.parse_expr()?;
                return Ok(Expr::Lambda {
                    params: params?,
                    body: Box::new(body),
                });
            }
            Ok(Expr::Tuple(exprs))
        } else {
            self.expect(Token::RParen)?;
            // Check for single-param lambda with parens: (x) -> expr
            if matches!(self.current(), Some(Token::Arrow)) {
                self.advance();
                let param = match expr {
                    Expr::Ident(n) => n,
                    _ => return Err("Lambda parameter must be an identifier".to_string()),
                };
                let body = self.parse_expr()?;
                return Ok(Expr::Lambda {
                    params: vec![param],
                    body: Box::new(body),
                });
            }
            Ok(expr)
        }
    }

    fn parse_list_literal(&mut self) -> Result<Expr, String> {
        self.advance(); // consume '['
        let mut exprs = Vec::new();
        while !matches!(self.current(), Some(Token::RBracket)) {
            exprs.push(self.parse_expr()?);
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(Token::RBracket)?;
        Ok(Expr::List(exprs))
    }

    fn parse_if_expr(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'if'
        let condition = self.parse_or_expr()?;
        self.expect(Token::ColonThen)?;
        self.skip_newlines();
        // Use parse_comparison_expr for then branch - allows binary ops but stops before :else
        let then_expr = self.parse_comparison_expr()?;
        self.skip_newlines();
        let else_expr = if matches!(self.current(), Some(Token::ColonElse)) {
            self.advance();
            self.skip_newlines();
            let e = self.parse_expr()?;
            Some(Box::new(e))
        } else {
            None
        };
        Ok(Expr::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_expr),
            else_branch: else_expr,
        })
    }

    fn parse_for_expr(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'for'
        let binding = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected identifier in for loop".to_string()),
        };
        self.expect(Token::In)?;
        let iterator = self.parse_expr()?;
        self.expect(Token::LBrace)?;
        self.skip_newlines();
        let body = self.parse_expr()?;
        self.skip_newlines();
        self.expect(Token::RBrace)?;
        Ok(Expr::For {
            binding,
            iterator: Box::new(iterator),
            body: Box::new(body),
        })
    }

    fn parse_ident_continuation(&mut self, n: String) -> Result<Expr, String> {
        // Check for pattern keywords (context-sensitive)
        // These are only patterns when followed by ( and have the right arg count
        if matches!(self.current(), Some(Token::LParen)) {
            match n.as_str() {
                "run" => {
                    self.advance(); // consume '('
                    let exprs = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    return Ok(Expr::Block(exprs));
                }
                "fold" | "map" | "filter" | "collect" | "recurse" | "parallel" => {
                    return self.parse_pattern_or_call_from_ident(&n);
                }
                _ => {} // Fall through to normal handling
            }
        }

        // Check for reassignment with = (for mutable bindings)
        if matches!(self.current(), Some(Token::Eq)) {
            // Make sure it's not == (equality check)
            self.advance();
            let value = self.parse_expr()?;
            return Ok(Expr::Reassign {
                target: n,
                value: Box::new(value),
            });
        }

        // Check for struct literal
        if matches!(self.current(), Some(Token::LBrace)) {
            self.advance();
            let mut fields = Vec::new();
            self.skip_newlines();
            while !matches!(self.current(), Some(Token::RBrace)) {
                let fname = match self.current() {
                    Some(Token::Ident(f)) => {
                        let f = f.clone();
                        self.advance();
                        f
                    }
                    _ => return Err("Expected field name".to_string()),
                };
                self.expect(Token::Colon)?;
                let value = self.parse_expr()?;
                fields.push((fname, value));

                self.skip_newlines();
                if matches!(self.current(), Some(Token::Comma)) {
                    self.advance();
                    self.skip_newlines();
                } else {
                    break;
                }
            }
            self.expect(Token::RBrace)?;
            return Ok(Expr::Struct { name: n, fields });
        }

        // Check for lambda: x -> expr
        if matches!(self.current(), Some(Token::Arrow)) {
            self.advance();
            let body = self.parse_expr()?;
            return Ok(Expr::Lambda {
                params: vec![n],
                body: Box::new(body),
            });
        }

        Ok(Expr::Ident(n))
    }

    fn parse_minus_expr(&mut self) -> Result<Expr, String> {
        // Could be unary minus or standalone operator
        // Check if followed by a number/expr
        self.advance();
        if matches!(
            self.current(),
            Some(Token::Int(_))
                | Some(Token::Float(_))
                | Some(Token::Ident(_))
                | Some(Token::LParen)
        ) {
            let operand = self.parse_primary_expr()?;
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
            })
        } else {
            Ok(Expr::Ident("-".to_string()))
        }
    }
}
