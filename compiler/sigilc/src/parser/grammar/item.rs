//! Item parsing (functions, tests, imports).
//!
//! This module extends Parser with methods for parsing top-level items
//! like function definitions, test definitions, and import statements.

use crate::ir::{
    Function, ImportPath, Param, ParamRange, TestDef, TokenKind, UseDef, UseItem,
};
use crate::parser::{FunctionOrTest, ParsedAttrs, ParseError, Parser};

impl<'a> Parser<'a> {
    /// Parse a use/import statement.
    /// Syntax: use './path' { item1, item2 as alias } or use std.math { sqrt }
    pub(in crate::parser) fn parse_use(&mut self) -> Result<UseDef, ParseError> {
        let start_span = self.current_span();
        self.expect(TokenKind::Use)?;

        // Parse import path
        let path = if let TokenKind::String(s) = self.current_kind() {
            // Relative path: './math', '../utils'
            self.advance();
            ImportPath::Relative(s)
        } else {
            // Module path: std.math, std.collections
            let mut segments = Vec::new();
            loop {
                let name = self.expect_ident()?;
                segments.push(name);

                if self.check(TokenKind::Dot) {
                    self.advance();
                } else {
                    break;
                }
            }
            ImportPath::Module(segments)
        };

        // Parse imported items: { item1, item2 as alias }
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            // Check for private import prefix ::
            let is_private = if self.check(TokenKind::DoubleColon) {
                self.advance();
                true
            } else {
                false
            };

            // Item name
            let name = self.expect_ident()?;

            // Optional alias: `as alias`
            let alias = if self.check(TokenKind::As) {
                self.advance();
                Some(self.expect_ident()?)
            } else {
                None
            };

            items.push(UseItem { name, alias, is_private });

            // Comma separator (optional before closing brace)
            if self.check(TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                self.skip_newlines();
                break;
            }
        }

        let end_span = self.current_span();
        self.expect(TokenKind::RBrace)?;

        Ok(UseDef {
            path,
            items,
            span: start_span.merge(end_span),
        })
    }


    /// Parse a function or test definition with attributes.
    ///
    /// Function: @name (params) -> Type = body
    /// Targeted test: @name tests @target1 tests @target2 (params) -> Type = body
    /// Free-floating test: @test_name (params) -> void = body
    pub(in crate::parser) fn parse_function_or_test_with_attrs(&mut self, attrs: ParsedAttrs) -> Result<FunctionOrTest, ParseError> {
        let start_span = self.current_span();

        // @
        self.expect(TokenKind::At)?;

        // name
        let name = self.expect_ident()?;
        let name_str = self.interner().lookup(name);
        let is_test_named = name_str.starts_with("test_");

        // Check if this is a targeted test (has `tests` keyword)
        if self.check(TokenKind::Tests) {
            // Parse test targets: tests @target1 tests @target2 ...
            let mut targets = Vec::new();
            while self.check(TokenKind::Tests) {
                self.advance(); // consume `tests`
                self.expect(TokenKind::At)?;
                let target = self.expect_ident()?;
                targets.push(target);
            }

            // (params)
            self.expect(TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(TokenKind::Eq)?;
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
                compile_fail_expected: attrs.compile_fail_expected,
                fail_expected: attrs.fail_expected,
            }))
        } else if is_test_named {
            // Free-floating test (name starts with test_ but no targets)
            // (params)
            self.expect(TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(TokenKind::Eq)?;
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
                compile_fail_expected: attrs.compile_fail_expected,
                fail_expected: attrs.fail_expected,
            }))
        } else {
            // Regular function
            // (params)
            self.expect(TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(TokenKind::RParen)?;

            // -> Type (optional)
            let return_ty = if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            // = body
            self.expect(TokenKind::Eq)?;
            let body = self.parse_expr()?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Function(Function {
                name,
                params,
                return_ty,
                body,
                span,
                is_public: false,
            }))
        }
    }

    /// Parse parameter list.
    pub(in crate::parser) fn parse_params(&mut self) -> Result<ParamRange, ParseError> {
        let mut params = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            let param_span = self.current_span();
            let name = self.expect_ident()?;

            // : Type (optional)
            let ty = if self.check(TokenKind::Colon) {
                self.advance();
                self.parse_type()
            } else {
                None
            };

            params.push(Param { name, ty, span: param_span });

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        Ok(self.arena.alloc_params(params))
    }
}
