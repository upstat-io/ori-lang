// Operator precedence parsing for Sigil
// Handles the operator precedence chain from lowest to highest:
// or < and < range < equality < comparison < additive < multiplicative < unary

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;

impl Parser {
    pub(super) fn parse_or_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
        let mut left = self.parse_and_expr()?;

        while matches!(self.current(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and_expr()?;
            let expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left.expr),
                right: Box::new(right.expr),
            };
            left = self.spanned(expr, start);
        }

        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
        let mut left = self.parse_range_expr()?;

        while matches!(self.current(), Some(Token::And)) {
            self.advance();
            let right = self.parse_range_expr()?;
            let expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left.expr),
                right: Box::new(right.expr),
            };
            left = self.spanned(expr, start);
        }

        Ok(left)
    }

    fn parse_range_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
        let left = self.parse_equality_expr()?;

        if matches!(self.current(), Some(Token::DotDot)) {
            self.advance();
            let right = self.parse_equality_expr()?;
            let expr = Expr::Range {
                start: Box::new(left.expr),
                end: Box::new(right.expr),
            };
            return Ok(self.spanned(expr, start));
        }

        Ok(left)
    }

    fn parse_equality_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
        let mut left = self.parse_comparison_expr()?;

        while matches!(self.current(), Some(Token::EqEq) | Some(Token::NotEq)) {
            let op = match self.current() {
                Some(Token::EqEq) => BinaryOp::Eq,
                Some(Token::NotEq) => BinaryOp::NotEq,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_comparison_expr()?;
            let expr = Expr::Binary {
                op,
                left: Box::new(left.expr),
                right: Box::new(right.expr),
            };
            left = self.spanned(expr, start);
        }

        Ok(left)
    }

    pub(super) fn parse_comparison_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
        let mut left = self.parse_additive_expr()?;

        while matches!(
            self.current(),
            Some(Token::Lt) | Some(Token::LtEq) | Some(Token::Gt) | Some(Token::GtEq)
        ) {
            let op = match self.current() {
                Some(Token::Lt) => BinaryOp::Lt,
                Some(Token::LtEq) => BinaryOp::LtEq,
                Some(Token::Gt) => BinaryOp::Gt,
                Some(Token::GtEq) => BinaryOp::GtEq,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_additive_expr()?;
            let expr = Expr::Binary {
                op,
                left: Box::new(left.expr),
                right: Box::new(right.expr),
            };
            left = self.spanned(expr, start);
        }

        Ok(left)
    }

    fn parse_additive_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
        let mut left = self.parse_multiplicative_expr()?;

        while matches!(self.current(), Some(Token::Plus) | Some(Token::Minus)) {
            let op = match self.current() {
                Some(Token::Plus) => BinaryOp::Add,
                Some(Token::Minus) => BinaryOp::Sub,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_multiplicative_expr()?;
            let expr = Expr::Binary {
                op,
                left: Box::new(left.expr),
                right: Box::new(right.expr),
            };
            left = self.spanned(expr, start);
        }

        Ok(left)
    }

    fn parse_multiplicative_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();
        let mut left = self.parse_unary_expr()?;

        while matches!(
            self.current(),
            Some(Token::Star) | Some(Token::Slash) | Some(Token::Percent) | Some(Token::Div)
        ) {
            let op = match self.current() {
                Some(Token::Star) => BinaryOp::Mul,
                Some(Token::Slash) => BinaryOp::Div,
                Some(Token::Percent) => BinaryOp::Mod,
                Some(Token::Div) => BinaryOp::IntDiv,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_unary_expr()?;
            let expr = Expr::Binary {
                op,
                left: Box::new(left.expr),
                right: Box::new(right.expr),
            };
            left = self.spanned(expr, start);
        }

        Ok(left)
    }

    pub(super) fn parse_unary_expr(&mut self) -> Result<SpannedExpr, String> {
        let start = self.current_start();

        if matches!(self.current(), Some(Token::Bang)) {
            self.advance();
            let operand = self.parse_unary_expr()?;
            let expr = Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand.expr),
            };
            return Ok(self.spanned(expr, start));
        }

        if matches!(self.current(), Some(Token::Minus)) {
            self.advance();
            let operand = self.parse_unary_expr()?;
            let expr = Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand.expr),
            };
            return Ok(self.spanned(expr, start));
        }

        self.parse_postfix_expr()
    }
}
