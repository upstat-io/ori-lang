//! Item parsing: functions, configs, types, traits, impls, tests.

use crate::intern::Name;
use crate::errors::Diagnostic;
use crate::syntax::{
    TokenKind, Span,
    items::{Item, ItemKind, Function, Config, Test, TypeParam, Visibility},
    expr::{Param, TypeExpr},
};
use super::Parser;

impl<'src, 'i> Parser<'src, 'i> {
    /// Parse a top-level item.
    pub(crate) fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        // Parse optional attributes: #[skip("reason")]
        let skip_reason = if self.check(&TokenKind::HashBracket) {
            Some(self.parse_skip_attribute()?)
        } else {
            None
        };

        let visibility = if self.check(&TokenKind::Pub) {
            self.advance();
            Visibility::Public
        } else {
            Visibility::Private
        };

        match self.current_kind() {
            TokenKind::At => self.parse_function(visibility, skip_reason),
            TokenKind::Dollar => {
                if skip_reason.is_some() {
                    return Err(self.error("#[skip] attribute can only be applied to tests"));
                }
                self.parse_config(visibility)
            }
            TokenKind::Type => {
                if skip_reason.is_some() {
                    return Err(self.error("#[skip] attribute can only be applied to tests"));
                }
                self.parse_type_def(visibility)
            }
            TokenKind::Use => {
                if skip_reason.is_some() {
                    return Err(self.error("#[skip] attribute can only be applied to tests"));
                }
                self.parse_import(visibility)
            }
            TokenKind::Trait => {
                if skip_reason.is_some() {
                    return Err(self.error("#[skip] attribute can only be applied to tests"));
                }
                self.parse_trait(visibility)
            }
            TokenKind::Impl => {
                if skip_reason.is_some() {
                    return Err(self.error("#[skip] attribute can only be applied to tests"));
                }
                self.parse_impl()
            }
            _ => Err(self.error("expected item declaration")),
        }
    }

    /// Parse #[skip("reason")] attribute.
    fn parse_skip_attribute(&mut self) -> Result<Name, Diagnostic> {
        self.consume(&TokenKind::HashBracket, "expected '#['")?;
        self.consume(&TokenKind::Skip, "expected 'skip'")?;
        self.consume(&TokenKind::LParen, "expected '('")?;

        let reason = match self.current_kind().clone() {
            TokenKind::String(name) => {
                self.advance();
                name
            }
            _ => return Err(self.error("expected string literal for skip reason")),
        };

        self.consume(&TokenKind::RParen, "expected ')'")?;
        self.consume(&TokenKind::RBracket, "expected ']'")?;
        self.skip_newlines();

        Ok(reason)
    }

    fn parse_function(&mut self, visibility: Visibility, skip_reason: Option<Name>) -> Result<Item, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::At, "expected '@'")?;

        let name = self.parse_name()?;

        // Check if this is a test declaration: @name tests @target
        if self.check(&TokenKind::Tests) {
            return self.parse_test(name, start, skip_reason);
        }

        // skip attribute can only be applied to tests
        if skip_reason.is_some() {
            return Err(self.error("#[skip] attribute can only be applied to tests"));
        }

        let type_params = self.parse_type_params()?;

        self.consume(&TokenKind::LParen, "expected '('")?;
        let params = self.parse_params()?;
        self.consume(&TokenKind::RParen, "expected ')'")?;

        let return_type = if self.check(&TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let capabilities = if self.check(&TokenKind::Uses) {
            self.advance();
            self.parse_capability_list()?
        } else {
            Vec::new()
        };

        self.consume(&TokenKind::Eq, "expected '='")?;
        self.skip_newlines();

        let body = self.expression()?;
        let span = start.merge(self.arena.get(body).span);

        Ok(Item {
            kind: ItemKind::Function(Function {
                name,
                visibility,
                type_params,
                params: self.arena.alloc_params(params),
                return_type: return_type.map(|t| self.arena.alloc_type_expr(t)),
                capabilities,
                body,
                is_async: false,
                sig_span: start,
            }),
            span,
        })
    }

    fn parse_test(&mut self, name: Name, start: Span, skip_reason: Option<Name>) -> Result<Item, Diagnostic> {
        // Parse: tests @target [tests @target2 ...] () -> void = body
        self.consume(&TokenKind::Tests, "expected 'tests'")?;

        let mut targets = Vec::new();

        // Parse first target
        self.consume(&TokenKind::At, "expected '@' after 'tests'")?;
        targets.push(self.parse_name()?);

        // Parse additional targets: tests @target2, tests @target3, etc.
        while self.check(&TokenKind::Tests) {
            self.advance();
            self.consume(&TokenKind::At, "expected '@' after 'tests'")?;
            targets.push(self.parse_name()?);
        }

        // Parse parameters (usually empty for tests)
        self.consume(&TokenKind::LParen, "expected '('")?;
        let _params = self.parse_params()?; // Ignore params for tests
        self.consume(&TokenKind::RParen, "expected ')'")?;

        // Parse return type (should be void)
        if self.check(&TokenKind::Arrow) {
            self.advance();
            let _return_type = self.parse_type_expr()?; // Ignore, should be void
        }

        // Parse body
        self.consume(&TokenKind::Eq, "expected '='")?;
        self.skip_newlines();

        let body = self.expression()?;
        let span = start.merge(self.arena.get(body).span);

        Ok(Item {
            kind: ItemKind::Test(Test {
                name,
                targets,
                body,
                skip_reason,
            }),
            span,
        })
    }

    fn parse_config(&mut self, visibility: Visibility) -> Result<Item, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::Dollar, "expected '$'")?;

        let name = self.parse_name()?;

        let ty = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        self.consume(&TokenKind::Eq, "expected '='")?;
        self.skip_newlines();

        let value = self.expression()?;
        let span = start.merge(self.arena.get(value).span);

        Ok(Item {
            kind: ItemKind::Config(Config {
                name,
                visibility,
                ty: ty.map(|t| self.arena.alloc_type_expr(t)),
                value,
            }),
            span,
        })
    }

    fn parse_type_def(&mut self, _visibility: Visibility) -> Result<Item, Diagnostic> {
        Err(self.error("type definitions not yet implemented"))
    }

    fn parse_import(&mut self, _visibility: Visibility) -> Result<Item, Diagnostic> {
        Err(self.error("imports not yet implemented"))
    }

    fn parse_trait(&mut self, _visibility: Visibility) -> Result<Item, Diagnostic> {
        Err(self.error("traits not yet implemented"))
    }

    fn parse_impl(&mut self) -> Result<Item, Diagnostic> {
        Err(self.error("impls not yet implemented"))
    }

    // ===== Item helper parsers =====

    pub(crate) fn parse_type_params(&mut self) -> Result<Vec<TypeParam>, Diagnostic> {
        if !self.check(&TokenKind::Lt) {
            return Ok(Vec::new());
        }

        self.advance();
        let mut params = Vec::new();

        loop {
            let name = self.parse_name()?;
            let start = self.current_span();

            let bounds = if self.check(&TokenKind::Colon) {
                self.advance();
                self.parse_type_bounds()?
            } else {
                Vec::new()
            };

            params.push(TypeParam {
                name,
                bounds,
                default: None,
                span: start,
            });

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        self.consume(&TokenKind::Gt, "expected '>'")?;
        Ok(params)
    }

    fn parse_type_bounds(&mut self) -> Result<Vec<TypeExpr>, Diagnostic> {
        let mut bounds = vec![self.parse_type_expr()?];

        while self.check(&TokenKind::Plus) {
            self.advance();
            bounds.push(self.parse_type_expr()?);
        }

        Ok(bounds)
    }

    pub(crate) fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.at_end() {
            let start = self.current_span();
            let name = self.parse_name()?;

            let ty = if self.check(&TokenKind::Colon) {
                self.advance();
                let type_expr = self.parse_type_expr()?;
                Some(self.arena.alloc_type_expr(type_expr))
            } else {
                None
            };

            let default = if self.check(&TokenKind::Eq) {
                self.advance();
                Some(self.expression()?)
            } else {
                None
            };

            params.push(Param {
                name,
                ty,
                default,
                span: start.merge(self.current_span()),
            });

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        Ok(params)
    }

    fn parse_capability_list(&mut self) -> Result<Vec<Name>, Diagnostic> {
        let mut caps = vec![self.parse_name()?];

        while self.check(&TokenKind::Comma) {
            self.advance();
            caps.push(self.parse_name()?);
        }

        Ok(caps)
    }
}
