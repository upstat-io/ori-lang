// Primary expression parsing for Sigil
// Handles literals, keywords, identifiers, control flow, lambdas, and structural expressions

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;

impl Parser {
    pub(super) fn parse_primary_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();

        match self.current() {
            Some(Token::Int(n)) => {
                let n = *n;
                self.advance();
                Ok(self.spanned(Expr::Int(n), start))
            }
            Some(Token::Float(f)) => {
                let f = *f;
                self.advance();
                Ok(self.spanned(Expr::Float(f), start))
            }
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                Ok(self.spanned(Expr::String(s), start))
            }
            Some(Token::True) => {
                self.advance();
                Ok(self.spanned(Expr::Bool(true), start))
            }
            Some(Token::False) => {
                self.advance();
                Ok(self.spanned(Expr::Bool(false), start))
            }
            Some(Token::Nil) => {
                self.advance();
                Ok(self.spanned(Expr::Nil, start))
            }
            Some(Token::Ok_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(self.spanned(Expr::Ok(Box::new(value.expr)), start))
            }
            Some(Token::Err_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(self.spanned(Expr::Err(Box::new(value.expr)), start))
            }
            Some(Token::Some_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(self.spanned(Expr::Some(Box::new(value.expr)), start))
            }
            Some(Token::None_) => {
                self.advance();
                Ok(self.spanned(Expr::None_, start))
            }
            Some(Token::Assert) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                let expr = Expr::Call {
                    func: Box::new(Expr::Ident("assert".to_string())),
                    args: args.into_iter().map(|a| a.expr).collect(),
                };
                Ok(self.spanned(expr, start))
            }
            Some(Token::AssertErr) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                let expr = Expr::Call {
                    func: Box::new(Expr::Ident("assert_err".to_string())),
                    args: args.into_iter().map(|a| a.expr).collect(),
                };
                Ok(self.spanned(expr, start))
            }
            // Type keywords used as conversion functions: str(), int(), etc.
            Some(Token::StrType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                let expr = Expr::Call {
                    func: Box::new(Expr::Ident("str".to_string())),
                    args: args.into_iter().map(|a| a.expr).collect(),
                };
                Ok(self.spanned(expr, start))
            }
            Some(Token::IntType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                let expr = Expr::Call {
                    func: Box::new(Expr::Ident("int".to_string())),
                    args: args.into_iter().map(|a| a.expr).collect(),
                };
                Ok(self.spanned(expr, start))
            }
            Some(Token::FloatType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                let expr = Expr::Call {
                    func: Box::new(Expr::Ident("float".to_string())),
                    args: args.into_iter().map(|a| a.expr).collect(),
                };
                Ok(self.spanned(expr, start))
            }
            Some(Token::BoolType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                let expr = Expr::Call {
                    func: Box::new(Expr::Ident("bool".to_string())),
                    args: args.into_iter().map(|a| a.expr).collect(),
                };
                Ok(self.spanned(expr, start))
            }
            Some(Token::Dollar) => {
                self.advance();
                match self.current() {
                    Some(Token::Ident(n)) => {
                        let n = n.clone();
                        self.advance();
                        Ok(self.spanned(Expr::Config(n), start))
                    }
                    _ => Err("Expected identifier after $".to_string()),
                }
            }
            Some(Token::Hash) => {
                // # is length placeholder (arr[# - 1] means arr[length - 1])
                self.advance();
                Ok(self.spanned(Expr::LengthPlaceholder, start))
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
                let expr = Expr::Let {
                    name,
                    mutable,
                    value: Box::new(value.expr),
                };
                Ok(self.spanned(expr, start))
            }
            Some(Token::With) => {
                // with Capability = implementation in body
                self.advance();
                let capability = match self.current() {
                    Some(Token::Ident(n)) => {
                        let n = n.clone();
                        self.advance();
                        n
                    }
                    _ => return Err("Expected capability name after 'with'".to_string()),
                };
                self.expect(Token::Eq)?;
                let implementation = self.parse_expr()?;
                self.expect(Token::In)?;
                let body = self.parse_expr()?;
                let expr = Expr::With {
                    capability,
                    implementation: Box::new(implementation.expr),
                    body: Box::new(body.expr),
                };
                Ok(self.spanned(expr, start))
            }
            Some(Token::Match) => {
                self.advance();
                self.parse_match_expr_with_start(start)
            }
            Some(Token::LParen) => self.parse_paren_expr_with_start(start),
            Some(Token::LBracket) => self.parse_list_literal_with_start(start),
            Some(Token::If) => self.parse_if_expr_with_start(start),
            Some(Token::For) => self.parse_for_expr_with_start(start),
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                self.parse_ident_continuation_with_start(n, start)
            }
            // Standalone operators as values (for fold, etc.)
            Some(Token::Plus) => {
                self.advance();
                Ok(self.spanned(Expr::Ident("+".to_string()), start))
            }
            Some(Token::Star) => {
                self.advance();
                Ok(self.spanned(Expr::Ident("*".to_string()), start))
            }
            Some(Token::Minus) => self.parse_minus_expr_with_start(start),
            _ => Err(format!(
                "Unexpected token in expression: {:?}",
                self.current()
            )),
        }
    }

    fn parse_paren_expr_with_start(&mut self, start: usize) -> Result<SpannedExpr, String> {
        self.advance(); // consume '('
        if matches!(self.current(), Some(Token::RParen)) {
            self.advance();
            // Check for lambda with no params: () -> expr
            if matches!(self.current(), Some(Token::Arrow)) {
                self.advance();
                let body = self.parse_expr()?;
                let expr = Expr::Lambda {
                    params: Vec::new(),
                    body: Box::new(body.expr),
                };
                return Ok(self.spanned(expr, start));
            }
            return Ok(self.spanned(Expr::Tuple(Vec::new()), start));
        }
        let inner_expr = self.parse_expr()?;
        if matches!(self.current(), Some(Token::Comma)) {
            // Could be tuple or multi-param lambda
            let mut exprs = vec![inner_expr];
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
                    .map(|e| match e.expr {
                        Expr::Ident(n) => Ok(n),
                        _ => Err("Lambda parameters must be identifiers".to_string()),
                    })
                    .collect();
                let body = self.parse_expr()?;
                let expr = Expr::Lambda {
                    params: params?,
                    body: Box::new(body.expr),
                };
                return Ok(self.spanned(expr, start));
            }
            let tuple_exprs: Vec<Expr> = exprs.into_iter().map(|e| e.expr).collect();
            Ok(self.spanned(Expr::Tuple(tuple_exprs), start))
        } else {
            self.expect(Token::RParen)?;
            // Check for single-param lambda with parens: (x) -> expr
            if matches!(self.current(), Some(Token::Arrow)) {
                self.advance();
                let param = match inner_expr.expr {
                    Expr::Ident(n) => n,
                    _ => return Err("Lambda parameter must be an identifier".to_string()),
                };
                let body = self.parse_expr()?;
                let expr = Expr::Lambda {
                    params: vec![param],
                    body: Box::new(body.expr),
                };
                return Ok(self.spanned(expr, start));
            }
            // Return the inner expression, but extend the span to include the parens
            Ok(SpannedExpr::new(inner_expr.expr, self.make_span(start)))
        }
    }

    fn parse_list_literal_with_start(&mut self, start: usize) -> Result<SpannedExpr, String> {
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
        let list_exprs: Vec<Expr> = exprs.into_iter().map(|e| e.expr).collect();
        Ok(self.spanned(Expr::List(list_exprs), start))
    }

    fn parse_if_expr_with_start(&mut self, start: usize) -> Result<SpannedExpr, String> {
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
            Some(Box::new(e.expr))
        } else {
            None
        };
        let expr = Expr::If {
            condition: Box::new(condition.expr),
            then_branch: Box::new(then_expr.expr),
            else_branch: else_expr,
        };
        Ok(self.spanned(expr, start))
    }

    fn parse_for_expr_with_start(&mut self, start: usize) -> Result<SpannedExpr, String> {
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
        let expr = Expr::For {
            binding,
            iterator: Box::new(iterator.expr),
            body: Box::new(body.expr),
        };
        Ok(self.spanned(expr, start))
    }

    fn parse_match_expr_with_start(&mut self, start: usize) -> Result<SpannedExpr, String> {
        // Delegate to patterns.rs parse_match_expr, which will need to be updated
        self.parse_match_expr_inner(start)
    }

    fn parse_ident_continuation_with_start(&mut self, n: String, start: usize) -> Result<SpannedExpr, String> {
        // Check for pattern keywords (context-sensitive)
        // These are only patterns when followed by ( and have the right arg count
        if matches!(self.current(), Some(Token::LParen)) {
            match n.as_str() {
                "run" => {
                    self.advance(); // consume '('
                    let exprs = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    let block_exprs: Vec<Expr> = exprs.into_iter().map(|e| e.expr).collect();
                    return Ok(self.spanned(Expr::Block(block_exprs), start));
                }
                "fold" | "map" | "filter" | "collect" | "recurse" | "parallel" => {
                    return self.parse_pattern_or_call_from_ident_with_start(&n, start);
                }
                _ => {} // Fall through to normal handling
            }
        }

        // Check for reassignment with = (for mutable bindings)
        if matches!(self.current(), Some(Token::Eq)) {
            // Make sure it's not == (equality check)
            self.advance();
            let value = self.parse_expr()?;
            let expr = Expr::Reassign {
                target: n,
                value: Box::new(value.expr),
            };
            return Ok(self.spanned(expr, start));
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
                fields.push((fname, value.expr));

                self.skip_newlines();
                if matches!(self.current(), Some(Token::Comma)) {
                    self.advance();
                    self.skip_newlines();
                } else {
                    break;
                }
            }
            self.expect(Token::RBrace)?;
            return Ok(self.spanned(Expr::Struct { name: n, fields }, start));
        }

        // Check for lambda: x -> expr
        if matches!(self.current(), Some(Token::Arrow)) {
            self.advance();
            let body = self.parse_expr()?;
            let expr = Expr::Lambda {
                params: vec![n],
                body: Box::new(body.expr),
            };
            return Ok(self.spanned(expr, start));
        }

        Ok(self.spanned(Expr::Ident(n), start))
    }

    fn parse_minus_expr_with_start(&mut self, start: usize) -> Result<SpannedExpr, String> {
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
            let expr = Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand.expr),
            };
            Ok(self.spanned(expr, start))
        } else {
            Ok(self.spanned(Expr::Ident("-".to_string()), start))
        }
    }
}
