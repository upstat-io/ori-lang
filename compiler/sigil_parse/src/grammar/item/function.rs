//! Function and test definition parsing.

use sigil_ir::{Function, GenericParamRange, Param, ParamRange, TestDef, TokenKind};
use crate::{FunctionOrTest, ParsedAttrs, ParseError, Parser};

impl Parser<'_> {
    /// Parse a function or test definition with attributes.
    ///
    /// Function: @name (params) -> Type = body
    /// Targeted test: @name tests @target1 tests @target2 (params) -> Type = body
    /// Free-floating test: @`test_name` (params) -> void = body
    pub(crate) fn parse_function_or_test_with_attrs(&mut self, attrs: ParsedAttrs, is_public: bool) -> Result<FunctionOrTest, ParseError> {
        let start_span = self.current_span();

        // @
        self.expect(&TokenKind::At)?;

        // name
        let name = self.expect_ident()?;
        let name_str = self.interner().lookup(name);
        let is_test_named = name_str.starts_with("test_");

        // Check if this is a targeted test (has `tests` keyword)
        if self.check(&TokenKind::Tests) {
            // Parse test targets: tests @target1 tests @target2 ...
            let mut targets = Vec::new();
            while self.check(&TokenKind::Tests) {
                self.advance(); // consume `tests`
                self.expect(&TokenKind::At)?;
                let target = self.expect_ident()?;
                targets.push(target);
            }

            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(&TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(&TokenKind::Eq)?;
            self.skip_newlines();
            let body = self.parse_expr()?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Test(TestDef {
                name,
                targets,
                params,
                return_ty,
                body,
                span,
                skip_reason: attrs.skip_reason,
                expected_errors: attrs.expected_errors,
                fail_expected: attrs.fail_expected,
            }))
        } else if is_test_named {
            // Free-floating test (name starts with test_ but no targets)
            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(&TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(&TokenKind::Eq)?;
            self.skip_newlines();
            let body = self.parse_expr()?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Test(TestDef {
                name,
                targets: Vec::new(), // No targets for free-floating tests
                params,
                return_ty,
                body,
                span,
                skip_reason: attrs.skip_reason,
                expected_errors: attrs.expected_errors,
                fail_expected: attrs.fail_expected,
            }))
        } else {
            // Regular function
            // Optional generic parameters: <T, U: Bound>
            let generics = if self.check(&TokenKind::Lt) {
                self.parse_generics()?
            } else {
                GenericParamRange::EMPTY
            };

            // (params)
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(&TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // Optional uses clause: uses Http, FileSystem
            let capabilities = if self.check(&TokenKind::Uses) {
                self.parse_uses_clause()?
            } else {
                Vec::new()
            };

            // Optional where clauses: where T: Clone, U: Default
            let where_clauses = if self.check(&TokenKind::Where) {
                self.parse_where_clauses()?
            } else {
                Vec::new()
            };

            // = body
            self.expect(&TokenKind::Eq)?;
            self.skip_newlines();
            let body = self.parse_expr()?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Function(Function {
                name,
                generics,
                params,
                return_ty,
                capabilities,
                where_clauses,
                body,
                span,
                is_public,
            }))
        }
    }

    /// Parse parameter list.
    /// Accepts both regular identifiers and `self` for trait methods.
    pub(crate) fn parse_params(&mut self) -> Result<ParamRange, ParseError> {
        let mut params = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            let param_span = self.current_span();

            // Accept `self` as a special parameter name for trait/impl methods
            let name = if self.check(&TokenKind::SelfLower) {
                self.advance();
                self.interner().intern("self")
            } else {
                self.expect_ident()?
            };

            // : Type (optional, not required for `self`)
            let ty = if self.check(&TokenKind::Colon) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            params.push(Param { name, ty, span: param_span });

            if !self.check(&TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
            }
        }

        Ok(self.arena.alloc_params(params))
    }
}
