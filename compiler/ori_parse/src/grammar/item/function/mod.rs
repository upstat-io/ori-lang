//! Function and test definition parsing.

use crate::context::ParseContext;
use crate::{committed, require, FunctionOrTest, ParseError, ParseOutcome, ParsedAttrs, Parser};
use ori_ir::{
    Function, GenericParamRange, Name, Param, ParamRange, Span, TestDef, TokenKind, Visibility,
};

impl Parser<'_> {
    /// Parse a function or test definition.
    ///
    /// Function: @name (params) -> Type = body
    /// Attached test: @name tests @target1 tests @target2 (params) -> Type = body
    /// Floating test: @name tests _ (params) -> void = body
    /// Floating test (legacy): @`test_name` (params) -> void = body
    ///
    /// Returns `EmptyErr` if no `@` is present.
    pub(crate) fn parse_function_or_test(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> ParseOutcome<FunctionOrTest> {
        if !self.cursor.check(&TokenKind::At) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::At,
                self.cursor.current_span().start as usize,
            );
        }

        self.in_error_context(crate::ErrorContext::FunctionDef, |p| {
            p.parse_function_or_test_body(attrs, visibility)
        })
    }

    fn parse_function_or_test_body(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
    ) -> ParseOutcome<FunctionOrTest> {
        let start_span = self.cursor.current_span();

        // @
        committed!(self.cursor.expect(&TokenKind::At));

        // name
        let name = committed!(self.cursor.expect_ident());

        // Check if this is a test (has `tests` keyword)
        // Grammar: test = "@" identifier "tests" test_targets "()" "->" "void" "=" expression
        //          test_targets = "_" | test_target { "tests" test_target }
        //          test_target  = "@" identifier
        if self.cursor.check(&TokenKind::Tests) {
            self.cursor.advance(); // consume initial `tests`

            // Parse test_targets: either `_` (floating) or `@target { tests @target }` (attached)
            let targets = if self.cursor.check(&TokenKind::Underscore) {
                self.cursor.advance(); // consume `_`
                Vec::new() // floating test — no targets
            } else {
                let mut targets = Vec::new();
                // First target (required)
                committed!(self.cursor.expect(&TokenKind::At));
                let target = committed!(self.cursor.expect_ident());
                targets.push(target);
                // Additional targets: `tests @target`
                while self.cursor.check(&TokenKind::Tests) {
                    self.cursor.advance(); // consume `tests`
                    committed!(self.cursor.expect(&TokenKind::At));
                    let target = committed!(self.cursor.expect_ident());
                    targets.push(target);
                }
                targets
            };

            self.parse_test_body(name, targets, attrs, start_span)
        } else if self.cursor.interner().lookup(name).starts_with("test_") {
            // Free-floating test (name starts with test_ but no targets)
            self.parse_test_body(name, Vec::new(), attrs, start_span)
        } else {
            // Regular function
            // Optional generic parameters: <T, U: Bound>
            let generics = if self.cursor.check(&TokenKind::Lt) {
                committed!(self.parse_generics().into_result())
            } else {
                GenericParamRange::EMPTY
            };

            // (params)
            committed!(self.cursor.expect(&TokenKind::LParen));
            let params = committed!(self.parse_params());
            committed!(self.cursor.expect(&TokenKind::RParen));

            // -> Type (required)
            committed!(self.cursor.expect(&TokenKind::Arrow));
            let return_ty = Some(committed!(self.parse_type_required().into_result()));

            // Optional uses clause: uses Http, FileSystem
            let capabilities = if self.cursor.check(&TokenKind::Uses) {
                committed!(self.parse_uses_clause().into_result())
            } else {
                Vec::new()
            };

            // Optional where clauses: where T: Clone, U: Default
            let where_clauses = if self.cursor.check(&TokenKind::Where) {
                committed!(self.parse_where_clauses().into_result())
            } else {
                Vec::new()
            };

            // Optional guard clause: if condition
            // Grammar: guard_clause = "if" expression
            // Use parse_non_assign_expr because `=` is the body delimiter, not assignment
            let guard = if self.cursor.check(&TokenKind::If) {
                self.cursor.advance(); // consume `if`
                Some(require!(
                    self,
                    self.parse_non_assign_expr(),
                    "guard expression after `if`"
                ))
            } else {
                None
            };

            // = body
            committed!(self.cursor.expect(&TokenKind::Eq));
            self.cursor.skip_newlines();
            let body = require!(
                self,
                self.with_context(ParseContext::IN_FUNCTION, Self::parse_expr),
                "function body"
            );

            let end_span = self.arena.get_expr(body).span;
            let span = start_span.merge(end_span);

            ParseOutcome::consumed_ok(FunctionOrTest::Function(Function {
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

    /// Parse the body of a test definition: `(params) -> Type = body`.
    ///
    /// Shared by both `tests _`/`tests @target` syntax and `test_` prefix detection.
    fn parse_test_body(
        &mut self,
        name: Name,
        targets: Vec<Name>,
        attrs: ParsedAttrs,
        start_span: Span,
    ) -> ParseOutcome<FunctionOrTest> {
        // (params)
        committed!(self.cursor.expect(&TokenKind::LParen));
        let params = committed!(self.parse_params());
        committed!(self.cursor.expect(&TokenKind::RParen));

        // -> Type (required)
        committed!(self.cursor.expect(&TokenKind::Arrow));
        let return_ty = Some(committed!(self.parse_type_required().into_result()));

        // = body
        committed!(self.cursor.expect(&TokenKind::Eq));
        self.cursor.skip_newlines();
        let body = require!(
            self,
            self.with_context(ParseContext::IN_FUNCTION, Self::parse_expr),
            "function body"
        );

        let end_span = self.arena.get_expr(body).span;
        let span = start_span.merge(end_span);

        ParseOutcome::consumed_ok(FunctionOrTest::Test(TestDef {
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
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive parameter parsing covering patterns, self, literals, defaults, and type annotations"
    )]
    pub(crate) fn parse_params(&mut self) -> Result<ParamRange, ParseError> {
        use crate::series::SeriesConfig;

        let start = self.arena.start_params();
        self.series_direct(&SeriesConfig::comma(TokenKind::RParen).no_newlines(), |p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(false);
            }

            let param_span = p.cursor.current_span();

            // Determine if this is a pattern or simple identifier
            let (name, pattern) = match *p.cursor.current_kind() {
                // `self` for trait/impl methods
                TokenKind::SelfLower => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("self"), None)
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
                    let gen_name = p.cursor.interner().intern("_arg");
                    (gen_name, Some(pat))
                }

                // List patterns for clause-based functions: [], [_, ..tail]
                TokenKind::LBracket => {
                    let pat = p.parse_match_pattern()?;
                    // Generate synthetic name for list patterns
                    let gen_name = p.cursor.interner().intern("_arg");
                    (gen_name, Some(pat))
                }

                // Negative literal: -42
                TokenKind::Minus => {
                    let pat = p.parse_match_pattern()?;
                    let gen_name = p.cursor.interner().intern("_arg");
                    (gen_name, Some(pat))
                }

                // Simple identifier (most common case)
                TokenKind::Ident(name) => {
                    p.cursor.advance();
                    (name, None)
                }

                // Context-sensitive keywords usable as parameter names
                TokenKind::Timeout => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("timeout"), None)
                }
                TokenKind::Parallel => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("parallel"), None)
                }
                TokenKind::Cache => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("cache"), None)
                }
                TokenKind::Catch => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("catch"), None)
                }
                TokenKind::Spawn => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("spawn"), None)
                }
                TokenKind::Recurse => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("recurse"), None)
                }
                TokenKind::Run => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("run"), None)
                }
                TokenKind::Try => {
                    p.cursor.advance();
                    (p.cursor.interner().intern("try"), None)
                }

                _ => {
                    return Err(ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        format!(
                            "expected parameter name or pattern, found {}",
                            p.cursor.current_kind().display_name()
                        ),
                        p.cursor.current_span(),
                    ));
                }
            };

            // : [...] Type (optional, not required for `self`)
            // Variadic syntax: `: ...Type`
            let (is_variadic, ty) = if p.cursor.check(&TokenKind::Colon) {
                p.cursor.advance();
                // Check for variadic: ...Type
                if p.cursor.check(&TokenKind::DotDotDot) {
                    p.cursor.advance();
                    (true, p.parse_type())
                } else {
                    (false, p.parse_type())
                }
            } else {
                (false, None)
            };

            // = default_value (optional)
            let default = if p.cursor.check(&TokenKind::Eq) {
                p.cursor.advance();
                Some(p.parse_expr().into_result()?)
            } else {
                None
            };

            let end_span = if default.is_some() || ty.is_some() {
                p.cursor.previous_span()
            } else {
                param_span
            };

            p.arena.push_param(Param {
                name,
                pattern,
                ty,
                default,
                is_variadic,
                span: param_span.merge(end_span),
            });
            Ok(true)
        })?;

        Ok(self.arena.finish_params(start))
    }
}

#[cfg(test)]
mod tests;
