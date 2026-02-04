//! Function and test definition parsing.

use crate::context::ParseContext;
use crate::{FunctionOrTest, ParseError, ParseResult, ParsedAttrs, Parser};
use ori_ir::{Function, GenericParamRange, Param, ParamRange, TestDef, TokenKind, Visibility};

impl Parser<'_> {
    /// Parse a function or test definition with progress tracking.
    pub(crate) fn parse_function_or_test_with_progress(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> ParseResult<FunctionOrTest> {
        self.with_progress(|p| p.parse_function_or_test_with_attrs(attrs, visibility))
    }

    /// Parse a function or test definition with attributes.
    ///
    /// Function: @name (params) -> Type = body
    /// Targeted test: @name tests @target1 tests @target2 (params) -> Type = body
    /// Free-floating test: @`test_name` (params) -> void = body
    pub(crate) fn parse_function_or_test_with_attrs(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> Result<FunctionOrTest, ParseError> {
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
            let body = self.with_context(ParseContext::IN_FUNCTION, Self::parse_expr)?;

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
            let body = self.with_context(ParseContext::IN_FUNCTION, Self::parse_expr)?;

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

            // Optional guard clause: if condition
            // Grammar: guard_clause = "if" expression
            // Use parse_non_assign_expr because `=` is the body delimiter, not assignment
            let guard = if self.check(&TokenKind::If) {
                self.advance(); // consume `if`
                Some(self.parse_non_assign_expr()?)
            } else {
                None
            };

            // = body
            self.expect(&TokenKind::Eq)?;
            self.skip_newlines();
            let body = self.with_context(ParseContext::IN_FUNCTION, Self::parse_expr)?;

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            Ok(FunctionOrTest::Function(Function {
                name,
                generics,
                params,
                return_ty,
                capabilities,
                where_clauses,
                guard,
                body,
                span,
                visibility,
            }))
        }
    }

    /// Parse parameter list with support for clause parameters.
    ///
    /// Grammar: `clause_param = match_pattern [ ":" type ] [ "=" expression ]`
    ///
    /// Supports:
    /// - Simple names: `(x: int)`
    /// - `self` for methods: `(self)`
    /// - Literal patterns: `(0: int)` — clause-based functions
    /// - List patterns: `([]: [T])`, `([_, ..tail]: [T])` — clause-based functions
    /// - Default values: `(x: int = 42)`
    pub(crate) fn parse_params(&mut self) -> Result<ParamRange, ParseError> {
        use crate::series::SeriesConfig;

        let params: Vec<Param> =
            self.series(&SeriesConfig::comma(TokenKind::RParen).no_newlines(), |p| {
                if p.check(&TokenKind::RParen) {
                    return Ok(None);
                }

                let param_span = p.current_span();

                // Determine if this is a pattern or simple identifier
                let (name, pattern) = match *p.current_kind() {
                    // `self` for trait/impl methods
                    TokenKind::SelfLower => {
                        p.advance();
                        (p.interner().intern("self"), None)
                    }

                    // Literal patterns for clause-based functions
                    TokenKind::Int(_)
                    | TokenKind::Float(_)
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::String(_)
                    | TokenKind::Char(_)
                    | TokenKind::Underscore => {
                        let pat = p.parse_match_pattern()?;
                        // Generate synthetic name for literal patterns
                        let gen_name = p.interner().intern("_arg");
                        (gen_name, Some(pat))
                    }

                    // List patterns for clause-based functions: [], [_, ..tail]
                    TokenKind::LBracket => {
                        let pat = p.parse_match_pattern()?;
                        // Generate synthetic name for list patterns
                        let gen_name = p.interner().intern("_arg");
                        (gen_name, Some(pat))
                    }

                    // Negative literal: -42
                    TokenKind::Minus => {
                        let pat = p.parse_match_pattern()?;
                        let gen_name = p.interner().intern("_arg");
                        (gen_name, Some(pat))
                    }

                    // Simple identifier (most common case)
                    TokenKind::Ident(name) => {
                        p.advance();
                        (name, None)
                    }

                    // Context-sensitive keywords usable as parameter names
                    TokenKind::Timeout => {
                        p.advance();
                        (p.interner().intern("timeout"), None)
                    }
                    TokenKind::Parallel => {
                        p.advance();
                        (p.interner().intern("parallel"), None)
                    }
                    TokenKind::Cache => {
                        p.advance();
                        (p.interner().intern("cache"), None)
                    }
                    TokenKind::Catch => {
                        p.advance();
                        (p.interner().intern("catch"), None)
                    }
                    TokenKind::Spawn => {
                        p.advance();
                        (p.interner().intern("spawn"), None)
                    }
                    TokenKind::Recurse => {
                        p.advance();
                        (p.interner().intern("recurse"), None)
                    }
                    TokenKind::Run => {
                        p.advance();
                        (p.interner().intern("run"), None)
                    }
                    TokenKind::Try => {
                        p.advance();
                        (p.interner().intern("try"), None)
                    }

                    _ => {
                        return Err(ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            format!(
                                "expected parameter name or pattern, found {}",
                                p.current_kind().display_name()
                            ),
                            p.current_span(),
                        ));
                    }
                };

                // : [...] Type (optional, not required for `self`)
                // Variadic syntax: `: ...Type`
                let (is_variadic, ty) = if p.check(&TokenKind::Colon) {
                    p.advance();
                    // Check for variadic: ...Type
                    if p.check(&TokenKind::DotDotDot) {
                        p.advance();
                        (true, p.parse_type())
                    } else {
                        (false, p.parse_type())
                    }
                } else {
                    (false, None)
                };

                // = default_value (optional)
                let default = if p.check(&TokenKind::Eq) {
                    p.advance();
                    Some(p.parse_expr()?)
                } else {
                    None
                };

                let end_span = if default.is_some() || ty.is_some() {
                    p.previous_span()
                } else {
                    param_span
                };

                Ok(Some(Param {
                    name,
                    pattern,
                    ty,
                    default,
                    is_variadic,
                    span: param_span.merge(end_span),
                }))
            })?;

        Ok(self.arena.alloc_params(params))
    }
}
