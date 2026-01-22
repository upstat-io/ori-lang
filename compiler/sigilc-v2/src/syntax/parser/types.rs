//! Type expression parsing.

use crate::errors::Diagnostic;
use crate::syntax::{
    TokenKind,
    expr::{TypeExpr, TypeExprKind},
};
use super::Parser;

impl<'src, 'i> Parser<'src, 'i> {
    /// Parse a type expression.
    pub(crate) fn parse_type_expr(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.current_span();

        match self.current_kind().clone() {
            TokenKind::IntType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("int"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::FloatType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("float"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::BoolType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("bool"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::StrType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("str"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::Void => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("void"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::Ident(name) => {
                self.advance();
                let type_args = if self.check(&TokenKind::Lt) {
                    self.parse_type_args()?
                } else {
                    Vec::new()
                };
                Ok(TypeExpr {
                    kind: TypeExprKind::Named { name, type_args },
                    span: span.merge(self.current_span()),
                })
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type_expr()?;
                self.consume(&TokenKind::RBracket, "expected ']'")?;
                Ok(TypeExpr {
                    kind: TypeExprKind::List(Box::new(inner)),
                    span: span.merge(self.current_span()),
                })
            }
            TokenKind::LParen => {
                self.advance();
                let mut types = Vec::new();

                while !self.check(&TokenKind::RParen) && !self.at_end() {
                    types.push(self.parse_type_expr()?);
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }

                self.consume(&TokenKind::RParen, "expected ')'")?;

                // Check for function type
                if self.check(&TokenKind::Arrow) {
                    self.advance();
                    let ret = self.parse_type_expr()?;
                    Ok(TypeExpr {
                        kind: TypeExprKind::Function {
                            params: types,
                            ret: Box::new(ret),
                        },
                        span: span.merge(self.current_span()),
                    })
                } else {
                    Ok(TypeExpr {
                        kind: TypeExprKind::Tuple(types),
                        span: span.merge(self.current_span()),
                    })
                }
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Infer,
                    span,
                })
            }
            TokenKind::LBrace => {
                // Map type: {K: V}
                self.advance();
                let key = self.parse_type_expr()?;
                self.consume(&TokenKind::Colon, "expected ':' in map type")?;
                let value = self.parse_type_expr()?;
                self.consume(&TokenKind::RBrace, "expected '}'")?;
                Ok(TypeExpr {
                    kind: TypeExprKind::Map {
                        key: Box::new(key),
                        value: Box::new(value),
                    },
                    span: span.merge(self.current_span()),
                })
            }
            _ => Err(self.error("expected type")),
        }
    }

    pub(crate) fn parse_type_args(&mut self) -> Result<Vec<TypeExpr>, Diagnostic> {
        self.consume(&TokenKind::Lt, "expected '<'")?;
        let mut args = Vec::new();

        loop {
            args.push(self.parse_type_expr()?);
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        // Use consume_gt_in_type to handle '>>' as two '>' for nested generics
        self.consume_gt_in_type()?;
        Ok(args)
    }
}
