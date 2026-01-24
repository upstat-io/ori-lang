//! Recursive descent parser for Sigil.
//!
//! Produces flat AST using ExprArena.

use crate::ir::{
    Name, Span, Token, TokenKind, TokenList,
    Expr, ExprKind, ExprArena, Module, Function, TestDef,
    BinaryOp, UnaryOp, Param, ParamRange,
    ExprId, ExprRange, StringInterner, TypeId, BindingPattern,
    FunctionSeq, FunctionExpKind, FunctionExp, SeqBinding, NamedExpr, CallArg,
    MatchArm, MatchPattern,
    UseDef, UseItem, ImportPath,
};

/// Result of parsing a definition starting with @.
/// Can be either a function or a test.
enum FunctionOrTest {
    Function(Function),
    Test(TestDef),
}

/// Parsed attributes for a function or test.
#[derive(Default)]
struct ParsedAttrs {
    skip_reason: Option<Name>,
    compile_fail_expected: Option<Name>,
    fail_expected: Option<Name>,
}

impl ParsedAttrs {
    fn is_empty(&self) -> bool {
        self.skip_reason.is_none()
            && self.compile_fail_expected.is_none()
            && self.fail_expected.is_none()
    }
}

/// Parser state.
pub struct Parser<'a> {
    tokens: &'a TokenList,
    interner: &'a StringInterner,
    pos: usize,
    arena: ExprArena,
}

impl<'a> Parser<'a> {
    /// Create a new parser.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        Parser {
            tokens,
            interner,
            pos: 0,
            arena: ExprArena::new(),
        }
    }

    /// Parse a module (collection of function definitions and tests).
    pub fn parse_module(mut self) -> ParseResult {
        let mut module = Module::new();
        let mut errors = Vec::new();

        // Parse imports first (must appear at beginning per spec)
        while !self.is_at_end() {
            self.skip_newlines();
            if self.is_at_end() {
                break;
            }

            if self.check(TokenKind::Use) {
                match self.parse_use() {
                    Ok(use_def) => module.imports.push(use_def),
                    Err(e) => {
                        self.recover_to_next_statement();
                        errors.push(e);
                    }
                }
            } else {
                // No more imports
                break;
            }
        }

        // Parse functions and tests
        while !self.is_at_end() {
            self.skip_newlines();

            if self.is_at_end() {
                break;
            }

            // Parse attributes before function/test definitions
            let attrs = self.parse_attributes(&mut errors);

            if self.check(TokenKind::At) {
                match self.parse_function_or_test_with_attrs(attrs) {
                    Ok(FunctionOrTest::Function(func)) => module.functions.push(func),
                    Ok(FunctionOrTest::Test(test)) => module.tests.push(test),
                    Err(e) => {
                        // Recovery: skip to next @ or EOF
                        self.recover_to_function();
                        errors.push(e);
                    }
                }
            } else if self.check(TokenKind::Use) {
                // Import after declarations - error
                errors.push(ParseError::new(
                    crate::diagnostic::ErrorCode::E1002,
                    "import statements must appear at the beginning of the file".to_string(),
                    self.current_span(),
                ));
                self.recover_to_next_statement();
            } else if !attrs.is_empty() {
                // Attributes without a following function/test
                errors.push(ParseError {
                    code: crate::diagnostic::ErrorCode::E1006,
                    message: "attributes must be followed by a function or test definition".to_string(),
                    span: self.current_span(),
                    context: None,
                });
                self.advance();
            } else {
                // Skip unknown token
                self.advance();
            }
        }

        ParseResult {
            module,
            arena: self.arena,
            errors,
        }
    }

    /// Parse a use/import statement.
    /// Syntax: use './path' { item1, item2 as alias } or use std.math { sqrt }
    fn parse_use(&mut self) -> Result<UseDef, ParseError> {
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

    /// Recovery: skip to next statement (@ or use or EOF)
    fn recover_to_next_statement(&mut self) {
        while !self.is_at_end() {
            if self.check(TokenKind::At) || self.check(TokenKind::Use) {
                return;
            }
            self.advance();
        }
    }

    /// Parse zero or more attributes: #[attr("value")]
    fn parse_attributes(&mut self, errors: &mut Vec<ParseError>) -> ParsedAttrs {
        let mut attrs = ParsedAttrs::default();

        while self.check(TokenKind::HashBracket) {
            self.advance(); // consume #[

            // Parse attribute name (can be identifier or specific keywords like `skip`)
            // We use an enum to track which attribute we're parsing
            #[derive(Clone, Copy)]
            enum AttrKind { Skip, CompileFail, Fail, Unknown }

            let attr_kind = match self.current_kind() {
                TokenKind::Ident(name) => {
                    let s = self.interner.lookup(name);
                    self.advance();
                    match s {
                        "skip" => AttrKind::Skip,
                        "compile_fail" => AttrKind::CompileFail,
                        "fail" => AttrKind::Fail,
                        _ => {
                            errors.push(ParseError::new(
                                crate::diagnostic::ErrorCode::E1006,
                                format!("unknown attribute '{}'", s),
                                self.previous_span(),
                            ));
                            AttrKind::Unknown
                        }
                    }
                }
                TokenKind::Skip => {
                    self.advance();
                    AttrKind::Skip
                }
                _ => {
                    errors.push(ParseError::new(
                        crate::diagnostic::ErrorCode::E1004,
                        format!("expected attribute name, found {:?}", self.current_kind()),
                        self.current_span(),
                    ));
                    // Try to recover by skipping to ]
                    while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                        self.advance();
                    }
                    if self.check(TokenKind::RBracket) {
                        self.advance();
                    }
                    continue;
                }
            };

            // For unknown attributes, skip to ] and continue
            if matches!(attr_kind, AttrKind::Unknown) {
                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    self.advance();
                }
                if self.check(TokenKind::RBracket) {
                    self.advance();
                }
                continue;
            }

            let attr_name_str = match attr_kind {
                AttrKind::Skip => "skip",
                AttrKind::CompileFail => "compile_fail",
                AttrKind::Fail => "fail",
                AttrKind::Unknown => unreachable!(),
            };

            // Expect (
            if !self.check(TokenKind::LParen) {
                errors.push(ParseError {
                    code: crate::diagnostic::ErrorCode::E1006,
                    message: format!("expected '(' after attribute name '{}'", attr_name_str),
                    span: self.current_span(),
                    context: None,
                });
                // Try to recover
                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    self.advance();
                }
                if self.check(TokenKind::RBracket) {
                    self.advance();
                }
                continue;
            }
            self.advance(); // consume (

            // Parse string value
            let value = if let TokenKind::String(string_name) = self.current_kind() {
                self.advance();
                Some(string_name)
            } else {
                errors.push(ParseError {
                    code: crate::diagnostic::ErrorCode::E1006,
                    message: format!("attribute '{}' requires a string argument", attr_name_str),
                    span: self.current_span(),
                    context: None,
                });
                None
            };

            // Expect )
            if !self.check(TokenKind::RParen) {
                errors.push(ParseError {
                    code: crate::diagnostic::ErrorCode::E1006,
                    message: "expected ')' after attribute value".to_string(),
                    span: self.current_span(),
                    context: None,
                });
            } else {
                self.advance();
            }

            // Expect ]
            if !self.check(TokenKind::RBracket) {
                errors.push(ParseError {
                    code: crate::diagnostic::ErrorCode::E1006,
                    message: "expected ']' to close attribute".to_string(),
                    span: self.current_span(),
                    context: None,
                });
            } else {
                self.advance();
            }

            // Store the attribute
            if let Some(value) = value {
                match attr_kind {
                    AttrKind::Skip => attrs.skip_reason = Some(value),
                    AttrKind::CompileFail => attrs.compile_fail_expected = Some(value),
                    AttrKind::Fail => attrs.fail_expected = Some(value),
                    AttrKind::Unknown => unreachable!(), // Already handled above
                }
            }

            self.skip_newlines();
        }

        attrs
    }

    /// Parse a function or test definition with attributes.
    ///
    /// Function: @name (params) -> Type = body
    /// Targeted test: @name tests @target1 tests @target2 (params) -> Type = body
    /// Free-floating test: @test_name (params) -> void = body
    fn parse_function_or_test_with_attrs(&mut self, attrs: ParsedAttrs) -> Result<FunctionOrTest, ParseError> {
        let start_span = self.current_span();

        // @
        self.expect(TokenKind::At)?;

        // name
        let name = self.expect_ident()?;
        let name_str = self.interner.lookup(name);
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
    fn parse_params(&mut self) -> Result<ParamRange, ParseError> {
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

    /// Parse a type expression.
    /// Returns Some(TypeId) for primitive types, None for unknown/complex types.
    fn parse_type(&mut self) -> Option<TypeId> {
        if self.check_type_keyword() {
            let kind = self.current().kind.clone();
            self.advance();
            match kind {
                TokenKind::IntType => Some(TypeId::INT),
                TokenKind::FloatType => Some(TypeId::FLOAT),
                TokenKind::BoolType => Some(TypeId::BOOL),
                TokenKind::StrType => Some(TypeId::STR),
                TokenKind::CharType => Some(TypeId::CHAR),
                TokenKind::ByteType => Some(TypeId::BYTE),
                TokenKind::Void => Some(TypeId::VOID),
                TokenKind::NeverType => Some(TypeId::NEVER),
                _ => None,
            }
        } else if self.check_ident() {
            // Named type - skip for now, return None
            // TODO: Look up user-defined types
            self.advance();
            None
        } else if self.check(TokenKind::LBracket) {
            // [T] list type - skip for now
            self.advance(); // [
            self.parse_type(); // inner type
            if self.check(TokenKind::RBracket) {
                self.advance(); // ]
            }
            // TODO: Return proper list type
            None
        } else if self.check(TokenKind::LParen) {
            // (T, U) tuple or () unit or () -> T function type
            self.advance(); // (
            if self.check(TokenKind::RParen) {
                self.advance(); // )
                // Check for -> (function type: () -> T)
                if self.check(TokenKind::Arrow) {
                    self.advance();
                    self.parse_type();
                    return None; // TODO: Return proper function type
                }
                return Some(TypeId::VOID); // () is unit/void
            }
            // Skip tuple contents
            while !self.check(TokenKind::RParen) && !self.is_at_end() {
                self.parse_type();
                if self.check(TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            if self.check(TokenKind::RParen) {
                self.advance();
            }
            // Check for -> (function type)
            if self.check(TokenKind::Arrow) {
                self.advance();
                self.parse_type();
            }
            None
        } else {
            None
        }
    }

    /// Parse an expression.
    /// Handles assignment at the top level: `identifier = expression`
    fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
        let left = self.parse_binary_or()?;

        // Check for assignment (= but not == or =>)
        if self.check(TokenKind::Eq) {
            let left_span = self.arena.get_expr(left).span;
            self.advance();
            let right = self.parse_expr()?;
            let right_span = self.arena.get_expr(right).span;
            let span = left_span.merge(right_span);
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Assign { target: left, value: right },
                span,
            )));
        }

        Ok(left)
    }

    /// Parse || (lowest precedence binary).
    fn parse_binary_or(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_binary_and()?;

        while self.check(TokenKind::PipePipe) {
            self.advance();
            let right = self.parse_binary_and()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::Or, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse && (logical and)
    fn parse_binary_and(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_or()?;

        while self.check(TokenKind::AmpAmp) {
            self.advance();
            let right = self.parse_bitwise_or()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::And, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse | (bitwise or)
    fn parse_bitwise_or(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_xor()?;

        while self.check(TokenKind::Pipe) {
            self.advance();
            let right = self.parse_bitwise_xor()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitOr, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse ^ (bitwise xor)
    fn parse_bitwise_xor(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_and()?;

        while self.check(TokenKind::Caret) {
            self.advance();
            let right = self.parse_bitwise_and()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitXor, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse & (bitwise and)
    fn parse_bitwise_and(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_equality()?;

        while self.check(TokenKind::Amp) {
            self.advance();
            let right = self.parse_equality()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitAnd, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse == and != (equality)
    fn parse_equality(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_comparison()?;

        while let Some(op) = self.match_equality_op() {
            self.advance();
            let right = self.parse_comparison()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse comparison operators (<, >, <=, >=).
    fn parse_comparison(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_range()?;

        while let Some(op) = self.match_comparison_op() {
            self.advance();
            let right = self.parse_range()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse range operators (.. and ..=).
    fn parse_range(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_shift()?;

        // Check for range operator
        if self.check(TokenKind::DotDot) || self.check(TokenKind::DotDotEq) {
            let inclusive = self.check(TokenKind::DotDotEq);
            self.advance();

            // Parse the end of the range (optional for open-ended ranges like 1..)
            let end = if self.check(TokenKind::Comma) || self.check(TokenKind::RParen) ||
                        self.check(TokenKind::RBracket) || self.is_at_end() {
                None
            } else {
                Some(self.parse_shift()?)
            };

            let span = if let Some(end_expr) = end {
                self.arena.get_expr(left).span.merge(self.arena.get_expr(end_expr).span)
            } else {
                self.arena.get_expr(left).span.merge(self.previous_span())
            };

            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Range { start: Some(left), end, inclusive },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse << and >> (shift operators).
    fn parse_shift(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_additive()?;

        while let Some(op) = self.match_shift_op() {
            self.advance();
            let right = self.parse_additive()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse + and -.
    fn parse_additive(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_multiplicative()?;

        while let Some(op) = self.match_additive_op() {
            self.advance();
            let right = self.parse_multiplicative()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse *, /, %.
    fn parse_multiplicative(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_unary()?;

        while let Some(op) = self.match_multiplicative_op() {
            self.advance();
            let right = self.parse_unary()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse unary operators.
    fn parse_unary(&mut self) -> Result<ExprId, ParseError> {
        if let Some(op) = self.match_unary_op() {
            let start = self.current_span();
            self.advance();
            let operand = self.parse_unary()?;

            let span = start.merge(self.arena.get_expr(operand).span);
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Unary { op, operand },
                span,
            )));
        }

        self.parse_call()
    }

    /// Parse function calls and field access.
    fn parse_call(&mut self) -> Result<ExprId, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(TokenKind::LParen) {
                // Function call
                self.advance();
                let (call_args, has_positional, has_named) = self.parse_call_args()?;
                self.expect(TokenKind::RParen)?;

                let call_span = self.arena.get_expr(expr).span.merge(self.previous_span());

                // Validate: multi-arg calls with positional args are an error
                if call_args.len() > 1 && has_positional {
                    return Err(ParseError::new(
                        crate::diagnostic::ErrorCode::E1011,
                        "function calls with multiple arguments require named arguments (.name: value)".to_string(),
                        call_span,
                    ));
                }

                // Choose representation based on whether we have named args
                if has_named {
                    // Use CallNamed for any call with named args
                    let args_range = self.arena.alloc_call_args(call_args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::CallNamed { func: expr, args: args_range },
                        call_span,
                    ));
                } else {
                    // Simple positional call (0 or 1 args)
                    let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
                    let args_range = self.arena.alloc_expr_list(args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Call { func: expr, args: args_range },
                        call_span,
                    ));
                }
            } else if self.check(TokenKind::Dot) {
                // Field access
                self.advance();
                let field = self.expect_ident()?;

                let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                expr = self.arena.alloc_expr(Expr::new(
                    ExprKind::Field { receiver: expr, field },
                    span,
                ));
            } else if self.check(TokenKind::LBracket) {
                // Index access
                self.advance();
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;

                let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                expr = self.arena.alloc_expr(Expr::new(
                    ExprKind::Index { receiver: expr, index },
                    span,
                ));
            } else if self.check(TokenKind::Arrow) {
                // Single-param lambda without parens: x -> body
                // Only valid if expr is a single identifier
                let expr_data = self.arena.get_expr(expr);
                if let ExprKind::Ident(name) = &expr_data.kind {
                    let param_span = expr_data.span;
                    let param_name = *name;
                    self.advance(); // consume ->
                    let body = self.parse_expr()?;
                    let end_span = self.arena.get_expr(body).span;
                    let params = self.arena.alloc_params(vec![Param {
                        name: param_name,
                        ty: None,
                        span: param_span,
                    }]);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Lambda { params, ret_ty: None, body },
                        param_span.merge(end_span),
                    ));
                }
                break;
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse function_seq: run or try with sequential bindings and statements.
    /// Grammar: run(let x = a, x = x + 1, result) or try(let x = fallible()?, Ok(x))
    fn parse_function_seq(&mut self, is_try: bool) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span(); // span of 'run' or 'try'
        self.expect(TokenKind::LParen)?;
        self.skip_newlines();

        let mut bindings = Vec::new();
        let mut result_expr = None;

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            // Check if this is a let binding
            if self.check(TokenKind::Let) {
                let binding_span = self.current_span();
                self.advance(); // consume 'let'

                // Check for 'mut'
                let mutable = if self.check(TokenKind::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };

                // Parse binding pattern
                let pattern = self.parse_binding_pattern()?;

                // Optional type annotation
                let ty = if self.check(TokenKind::Colon) {
                    self.advance();
                    self.parse_type()
                } else {
                    None
                };

                // Expect '='
                self.expect(TokenKind::Eq)?;

                // Parse value
                let value = self.parse_expr()?;
                let end_span = self.arena.get_expr(value).span;

                bindings.push(SeqBinding::Let {
                    pattern,
                    ty,
                    value,
                    mutable,
                    span: binding_span.merge(end_span),
                });
            } else {
                // Parse an expression
                let expr_span = self.current_span();
                let expr = self.parse_expr()?;
                let end_span = self.arena.get_expr(expr).span;

                self.skip_newlines();

                // Check what comes after to determine if this is a statement or result
                if self.check(TokenKind::Comma) {
                    self.advance(); // consume comma
                    self.skip_newlines();

                    // If the next token is ), this was a trailing comma and expr is the result
                    if self.check(TokenKind::RParen) {
                        result_expr = Some(expr);
                    } else {
                        // There's more content, so this is a statement expression
                        bindings.push(SeqBinding::Stmt {
                            expr,
                            span: expr_span.merge(end_span),
                        });
                    }
                    continue;
                } else {
                    // No comma, this is the result expression
                    result_expr = Some(expr);
                }
            }

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(TokenKind::RParen)?;
        let end_span = self.previous_span();

        // Result expression is required
        let result = result_expr.ok_or_else(|| {
            ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("{} requires a result expression", if is_try { "try" } else { "run" }),
                end_span,
            )
        })?;

        let bindings_range = self.arena.alloc_seq_bindings(bindings);
        let span = start_span.merge(end_span);
        let func_seq = if is_try {
            FunctionSeq::Try { bindings: bindings_range, result, span }
        } else {
            FunctionSeq::Run { bindings: bindings_range, result, span }
        };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionSeq(func_seq),
            span,
        )))
    }

    /// Parse match as function_seq: match(scrutinee, Pattern -> expr, ...)
    fn parse_match_expr(&mut self) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span(); // span of 'match'
        self.expect(TokenKind::LParen)?;
        self.skip_newlines();

        // First argument is the scrutinee
        let scrutinee = self.parse_expr()?;

        self.skip_newlines();
        self.expect(TokenKind::Comma)?;
        self.skip_newlines();

        // Parse match arms: Pattern -> expr
        let mut arms = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            let arm_span = self.current_span();
            let pattern = self.parse_match_pattern()?;

            self.expect(TokenKind::Arrow)?;
            let body = self.parse_expr()?;
            let end_span = self.arena.get_expr(body).span;

            arms.push(MatchArm {
                pattern,
                guard: None, // TODO: add guard support
                body,
                span: arm_span.merge(end_span),
            });

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(TokenKind::RParen)?;
        let end_span = self.previous_span();

        if arms.is_empty() {
            return Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                "match requires at least one arm".to_string(),
                end_span,
            ));
        }

        let arms_range = self.arena.alloc_arms(arms);
        let span = start_span.merge(end_span);
        let func_seq = FunctionSeq::Match { scrutinee, arms: arms_range, span };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionSeq(func_seq),
            span,
        )))
    }

    /// Parse a match pattern (for match arms).
    fn parse_match_pattern(&mut self) -> Result<MatchPattern, ParseError> {
        match self.current_kind() {
            TokenKind::Underscore => {
                self.advance();
                Ok(MatchPattern::Wildcard)
            }
            TokenKind::Int(n) => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Int(n),
                    self.previous_span(),
                ))))
            }
            TokenKind::True => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Bool(true),
                    self.previous_span(),
                ))))
            }
            TokenKind::False => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Bool(false),
                    self.previous_span(),
                ))))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::String(name),
                    self.previous_span(),
                ))))
            }
            TokenKind::Ident(name) => {
                self.advance();
                // Check if this is a variant pattern like Some(x) or just a binding
                if self.check(TokenKind::LParen) {
                    // Variant pattern with optional inner pattern
                    self.advance();
                    let inner = if self.check(TokenKind::RParen) {
                        None
                    } else {
                        let pat = self.parse_match_pattern()?;
                        Some(Box::new(pat))
                    };
                    self.expect(TokenKind::RParen)?;
                    Ok(MatchPattern::Variant { name, inner })
                } else {
                    // Simple binding
                    Ok(MatchPattern::Binding(name))
                }
            }
            // Option variants
            TokenKind::Some => {
                let name = self.interner.intern("Some");
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = if self.check(TokenKind::RParen) {
                    None
                } else {
                    let pat = self.parse_match_pattern()?;
                    Some(Box::new(pat))
                };
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::None => {
                let name = self.interner.intern("None");
                self.advance();
                Ok(MatchPattern::Variant { name, inner: None })
            }
            // Result variants
            TokenKind::Ok => {
                let name = self.interner.intern("Ok");
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = if self.check(TokenKind::RParen) {
                    None
                } else {
                    let pat = self.parse_match_pattern()?;
                    Some(Box::new(pat))
                };
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::Err => {
                let name = self.interner.intern("Err");
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = if self.check(TokenKind::RParen) {
                    None
                } else {
                    let pat = self.parse_match_pattern()?;
                    Some(Box::new(pat))
                };
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::LParen => {
                // Tuple pattern
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    patterns.push(self.parse_match_pattern()?);
                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Tuple(patterns))
            }
            _ => Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("expected match pattern, found {:?}", self.current_kind()),
                self.current_span(),
            )),
        }
    }

    /// Parse function_exp: map, filter, fold, etc. with named properties.
    /// Grammar: kind(.prop1: expr1, .prop2: expr2, ...)
    fn parse_function_exp(&mut self, kind: FunctionExpKind) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span(); // span of the keyword
        self.expect(TokenKind::LParen)?;
        self.skip_newlines();

        let mut props = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            // Require named property: .name: expr
            if !self.check(TokenKind::Dot) {
                return Err(ParseError::new(
                    crate::diagnostic::ErrorCode::E1013,
                    format!("`{}` requires named properties (.name: value)", kind.name()),
                    self.current_span(),
                ));
            }

            self.advance(); // consume '.'
            let name = self.expect_ident_or_keyword()?;
            let prop_span = self.previous_span();
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr()?;
            let end_span = self.arena.get_expr(value).span;

            props.push(NamedExpr {
                name,
                value,
                span: prop_span.merge(end_span),
            });

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(TokenKind::RParen)?;
        let end_span = self.previous_span();

        let props_range = self.arena.alloc_named_exprs(props);
        let func_exp = FunctionExp {
            kind,
            props: props_range,
            span: start_span.merge(end_span),
        };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionExp(func_exp),
            start_span.merge(end_span),
        )))
    }

    /// Parse call arguments, supporting both positional and named args.
    /// Returns (args, has_positional, has_named).
    fn parse_call_args(&mut self) -> Result<(Vec<CallArg>, bool, bool), ParseError> {
        let mut args = Vec::new();
        let mut has_positional = false;
        let mut has_named = false;

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            let arg_span = self.current_span();

            // Check for named argument: .name: expr
            if self.check(TokenKind::Dot) {
                self.advance(); // consume '.'
                let name = self.expect_ident_or_keyword()?;
                self.expect(TokenKind::Colon)?;
                let value = self.parse_expr()?;
                let end_span = self.arena.get_expr(value).span;

                args.push(CallArg {
                    name: Some(name),
                    value,
                    span: arg_span.merge(end_span),
                });
                has_named = true;
            } else {
                // Positional argument
                let value = self.parse_expr()?;
                let end_span = self.arena.get_expr(value).span;

                args.push(CallArg {
                    name: None,
                    value,
                    span: arg_span.merge(end_span),
                });
                has_positional = true;
            }

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();

        Ok((args, has_positional, has_named))
    }

    /// Parse primary expressions.
    fn parse_primary(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();

        // function_seq keywords (run, try)
        if let Some(is_try) = self.match_function_seq_kind() {
            self.advance();
            return self.parse_function_seq(is_try);
        }

        // match is also function_seq but parsed separately
        if self.check(TokenKind::Match) {
            self.advance();
            return self.parse_match_expr();
        }

        // function_exp keywords (map, filter, fold, etc.)
        if let Some(kind) = self.match_function_exp_kind() {
            self.advance();
            return self.parse_function_exp(kind);
        }

        match self.current_kind() {
            // Literals
            TokenKind::Int(n) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Int(n), span)))
            }
            TokenKind::Float(bits) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Float(bits), span)))
            }
            TokenKind::True => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Bool(true), span)))
            }
            TokenKind::False => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Bool(false), span)))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::String(name), span)))
            }
            TokenKind::Char(c) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Char(c), span)))
            }
            TokenKind::Duration(value, unit) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Duration { value, unit }, span)))
            }
            TokenKind::Size(value, unit) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Size { value, unit }, span)))
            }

            // Identifier
            TokenKind::Ident(name) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Soft keywords used as identifiers (when not followed by `(`)
            // These are context-sensitive: `len(` is a built-in call, but `let len = 5` is a variable
            TokenKind::Len | TokenKind::Min | TokenKind::Max | TokenKind::Compare |
            TokenKind::IsEmpty | TokenKind::IsSome | TokenKind::IsNone |
            TokenKind::IsOk | TokenKind::IsErr | TokenKind::Print | TokenKind::Panic |
            TokenKind::Assert | TokenKind::AssertEq | TokenKind::AssertNe => {
                // This branch is only reached when NOT followed by `(`, since
                // match_function_exp_kind handles the `keyword(` case first.
                let name_str = self.soft_keyword_to_name().expect("soft keyword matched but not in helper");
                let name = self.interner.intern(name_str);
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // self - used in recurse pattern for recursive calls
            TokenKind::SelfLower => {
                self.advance();
                let name = self.interner.intern("self");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Type keywords used as function_val conversion functions: int(x), float(x), str(x), etc.
            // Per spec, these are prelude functions that can be called in expression context.
            TokenKind::IntType => {
                self.advance();
                let name = self.interner.intern("int");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::FloatType => {
                self.advance();
                let name = self.interner.intern("float");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::StrType => {
                self.advance();
                let name = self.interner.intern("str");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::BoolType => {
                self.advance();
                let name = self.interner.intern("bool");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::CharType => {
                self.advance();
                let name = self.interner.intern("char");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::ByteType => {
                self.advance();
                let name = self.interner.intern("byte");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Variant constructors: Some(x), None, Ok(x), Err(x)
            TokenKind::Some => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = self.parse_expr()?;
                let end_span = self.current_span();
                self.expect(TokenKind::RParen)?;
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Some(inner),
                    span.merge(end_span),
                )))
            }
            TokenKind::None => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::None, span)))
            }
            TokenKind::Ok => {
                self.advance();
                let inner = if self.check(TokenKind::LParen) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
                    Some(expr)
                } else {
                    None
                };
                let end_span = self.previous_span();
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Ok(inner),
                    span.merge(end_span),
                )))
            }
            TokenKind::Err => {
                self.advance();
                let inner = if self.check(TokenKind::LParen) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
                    Some(expr)
                } else {
                    None
                };
                let end_span = self.previous_span();
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Err(inner),
                    span.merge(end_span),
                )))
            }

            // Parenthesized expression, tuple, or lambda
            TokenKind::LParen => {
                self.advance();

                // Case 1: () -> body (lambda with no params)
                if self.check(TokenKind::RParen) {
                    self.advance();

                    // Check for arrow to determine if lambda or unit
                    if self.check(TokenKind::Arrow) {
                        self.advance();
                        // Optional return type
                        let ret_ty = if self.check_type_keyword() {
                            let ty = self.parse_type();
                            self.expect(TokenKind::Eq)?;
                            ty
                        } else {
                            None
                        };
                        let body = self.parse_expr()?;
                        let end_span = self.arena.get_expr(body).span;
                        return Ok(self.arena.alloc_expr(Expr::new(
                            ExprKind::Lambda {
                                params: ParamRange::EMPTY,
                                ret_ty,
                                body
                            },
                            span.merge(end_span),
                        )));
                    }

                    // Unit: ()
                    let end_span = self.previous_span();
                    return Ok(self.arena.alloc_expr(Expr::new(
                        ExprKind::Tuple(ExprRange::EMPTY),
                        span.merge(end_span),
                    )));
                }

                // Case 2: Check for typed lambda params: (ident : Type, ...)
                // If we see "ident :" pattern, parse as lambda params
                if self.is_typed_lambda_params() {
                    let params = self.parse_params()?;
                    self.expect(TokenKind::RParen)?;
                    self.expect(TokenKind::Arrow)?;

                    // Optional return type: (x: int) -> int = body
                    let ret_ty = if self.check_type_keyword() {
                        let ty = self.parse_type();
                        self.expect(TokenKind::Eq)?;
                        ty
                    } else {
                        None
                    };

                    let body = self.parse_expr()?;
                    let end_span = self.arena.get_expr(body).span;
                    return Ok(self.arena.alloc_expr(Expr::new(
                        ExprKind::Lambda { params, ret_ty, body },
                        span.merge(end_span),
                    )));
                }

                // Case 3: Untyped - parse as expression(s), then check for ->
                let expr = self.parse_expr()?;

                if self.check(TokenKind::Comma) {
                    // Tuple or untyped multi-param lambda
                    let mut exprs = vec![expr];
                    while self.check(TokenKind::Comma) {
                        self.advance();
                        if self.check(TokenKind::RParen) {
                            break;
                        }
                        exprs.push(self.parse_expr()?);
                    }
                    self.expect(TokenKind::RParen)?;

                    // Check for arrow - if present, convert to lambda
                    if self.check(TokenKind::Arrow) {
                        self.advance();
                        let params = self.exprs_to_params(&exprs)?;
                        let body = self.parse_expr()?;
                        let end_span = self.arena.get_expr(body).span;
                        return Ok(self.arena.alloc_expr(Expr::new(
                            ExprKind::Lambda { params, ret_ty: None, body },
                            span.merge(end_span),
                        )));
                    }

                    let end_span = self.previous_span();
                    let range = self.arena.alloc_expr_list(exprs);
                    return Ok(self.arena.alloc_expr(Expr::new(
                        ExprKind::Tuple(range),
                        span.merge(end_span),
                    )));
                }

                self.expect(TokenKind::RParen)?;

                // Check for arrow - single param untyped lambda: (x) -> body
                if self.check(TokenKind::Arrow) {
                    self.advance();
                    let params = self.exprs_to_params(&[expr])?;
                    let body = self.parse_expr()?;
                    let end_span = self.arena.get_expr(body).span;
                    return Ok(self.arena.alloc_expr(Expr::new(
                        ExprKind::Lambda { params, ret_ty: None, body },
                        span.merge(end_span),
                    )));
                }

                Ok(expr)
            }

            // List literal
            TokenKind::LBracket => {
                self.advance();
                let mut exprs = Vec::new();

                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    exprs.push(self.parse_expr()?);
                    if !self.check(TokenKind::RBracket) {
                        self.expect(TokenKind::Comma)?;
                    }
                }

                self.expect(TokenKind::RBracket)?;
                let end_span = self.previous_span();
                let range = self.arena.alloc_expr_list(exprs);
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::List(range),
                    span.merge(end_span),
                )))
            }

            // If expression
            TokenKind::If => {
                self.advance();
                let cond = self.parse_expr()?;
                self.expect(TokenKind::Then)?;
                let then_branch = self.parse_expr()?;

                // Skip newlines before checking for else (allows multiline if-else)
                self.skip_newlines();

                let else_branch = if self.check(TokenKind::Else) {
                    self.advance();
                    Some(self.parse_expr()?)
                } else {
                    None
                };

                let end_span = if let Some(else_id) = else_branch {
                    self.arena.get_expr(else_id).span
                } else {
                    self.arena.get_expr(then_branch).span
                };

                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::If { cond, then_branch, else_branch },
                    span.merge(end_span),
                )))
            }

            // Let expression: let [mut] pattern [: type] = init
            TokenKind::Let => {
                self.advance();

                // Check for 'mut' keyword
                let mutable = if self.check(TokenKind::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };

                // Parse binding pattern (simplified to just name for now)
                let pattern = self.parse_binding_pattern()?;

                // Optional type annotation
                let ty = if self.check(TokenKind::Colon) {
                    self.advance();
                    self.parse_type()
                } else {
                    None
                };

                // Expect '='
                self.expect(TokenKind::Eq)?;

                // Parse initializer
                let init = self.parse_expr()?;

                let end_span = self.arena.get_expr(init).span;
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Let { pattern, ty, init, mutable },
                    span.merge(end_span),
                )))
            }

            _ => Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("expected expression, found {:?}", self.current_kind()),
                span,
            )),
        }
    }

    // ===== Helper methods =====

    /// Parse a binding pattern (for let expressions).
    ///
    /// Currently supports:
    /// - Simple name: `x`
    /// - Wildcard: `_`
    /// - Tuple: `(a, b, c)`
    fn parse_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        // Check for soft keywords first (len, min, max, etc. can be used as variable names)
        if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner.intern(name_str);
            self.advance();
            return Ok(BindingPattern::Name(name));
        }

        match self.current_kind() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(BindingPattern::Name(name))
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(BindingPattern::Wildcard)
            }
            TokenKind::LParen => {
                // Tuple pattern
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    patterns.push(self.parse_binding_pattern()?);
                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(BindingPattern::Tuple(patterns))
            }
            _ => Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("expected binding pattern, found {:?}", self.current_kind()),
                self.current_span(),
            )),
        }
    }

    /// Check if we're looking at typed lambda params: (ident : Type, ...)
    /// This looks ahead to detect the "ident :" pattern that distinguishes
    /// typed lambda params from expressions.
    fn is_typed_lambda_params(&self) -> bool {
        // Look at first token - must be an identifier or soft keyword
        let is_ident_like = matches!(self.current_kind(), TokenKind::Ident(_))
            || self.soft_keyword_to_name().is_some();
        if !is_ident_like {
            return false;
        }
        // Look at second token - must be a colon
        if self.pos + 1 < self.tokens.len() {
            matches!(self.tokens[self.pos + 1].kind, TokenKind::Colon)
        } else {
            false
        }
    }

    /// Convert parsed expressions to lambda parameters.
    /// Each expression must be an identifier.
    fn exprs_to_params(&mut self, exprs: &[ExprId]) -> Result<ParamRange, ParseError> {
        let mut params = Vec::new();
        for &expr_id in exprs {
            let expr = self.arena.get_expr(expr_id);
            match &expr.kind {
                ExprKind::Ident(name) => {
                    params.push(Param {
                        name: *name,
                        ty: None,
                        span: expr.span,
                    });
                }
                _ => {
                    return Err(ParseError::new(
                        crate::diagnostic::ErrorCode::E1002,
                        "expected identifier for lambda parameter".to_string(),
                        expr.span,
                    ));
                }
            }
        }
        Ok(self.arena.alloc_params(params))
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&self.tokens[self.tokens.len() - 1])
    }

    fn current_kind(&self) -> TokenKind {
        self.current().kind.clone()
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    fn previous_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span::DUMMY
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    fn check(&self, kind: TokenKind) -> bool {
        std::mem::discriminant(&self.current_kind()) == std::mem::discriminant(&kind)
    }

    fn check_ident(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Ident(_))
    }

    fn check_type_keyword(&self) -> bool {
        matches!(
            self.current_kind(),
            TokenKind::IntType | TokenKind::FloatType | TokenKind::BoolType |
            TokenKind::StrType | TokenKind::CharType | TokenKind::ByteType |
            TokenKind::Void | TokenKind::NeverType
        )
    }

    /// Check if the next token (lookahead) is a left paren.
    fn next_is_lparen(&self) -> bool {
        self.pos + 1 < self.tokens.len() && matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen)
    }

    /// Check if current token is a context-sensitive built-in keyword that can be used as an identifier.
    /// These are built-ins that are only treated as keywords when followed by `(`.
    /// Returns the interned name if it's a soft keyword, None otherwise.
    fn soft_keyword_to_name(&self) -> Option<&'static str> {
        match self.current_kind() {
            TokenKind::Len => Some("len"),
            TokenKind::Min => Some("min"),
            TokenKind::Max => Some("max"),
            TokenKind::Compare => Some("compare"),
            TokenKind::IsEmpty => Some("is_empty"),
            TokenKind::IsSome => Some("is_some"),
            TokenKind::IsNone => Some("is_none"),
            TokenKind::IsOk => Some("is_ok"),
            TokenKind::IsErr => Some("is_err"),
            TokenKind::Print => Some("print"),
            TokenKind::Panic => Some("panic"),
            TokenKind::Assert => Some("assert"),
            TokenKind::AssertEq => Some("assert_eq"),
            TokenKind::AssertNe => Some("assert_ne"),
            _ => None,
        }
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        &self.tokens[self.pos - 1]
    }

    fn skip_newlines(&mut self) {
        while self.check(TokenKind::Newline) {
            self.advance();
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<&Token, ParseError> {
        if self.check(kind.clone()) {
            Ok(self.advance())
        } else {
            Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1001,
                format!("expected {:?}, found {:?}", kind, self.current_kind()),
                self.current_span(),
            ).with_context(format!("expected {:?}", kind)))
        }
    }

    fn expect_ident(&mut self) -> Result<Name, ParseError> {
        // Accept regular identifiers
        if let TokenKind::Ident(name) = self.current_kind() {
            self.advance();
            Ok(name)
        // Also accept soft keywords as identifiers (len, min, max, etc.)
        } else if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner.intern(name_str);
            self.advance();
            Ok(name)
        } else {
            Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1004,
                format!("expected identifier, found {:?}", self.current_kind()),
                self.current_span(),
            ))
        }
    }

    /// Accept an identifier or a keyword that can be used as a named argument name.
    /// This handles cases like `.where:` in the find pattern where `where` is a keyword.
    fn expect_ident_or_keyword(&mut self) -> Result<Name, ParseError> {
        match self.current_kind() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            // Keywords that can be used as named argument names
            TokenKind::Where => {
                self.advance();
                Ok(self.interner.intern("where"))
            }
            TokenKind::Match => {
                self.advance();
                Ok(self.interner.intern("match"))
            }
            TokenKind::For => {
                self.advance();
                Ok(self.interner.intern("for"))
            }
            TokenKind::In => {
                self.advance();
                Ok(self.interner.intern("in"))
            }
            TokenKind::If => {
                self.advance();
                Ok(self.interner.intern("if"))
            }
            TokenKind::Type => {
                self.advance();
                Ok(self.interner.intern("type"))
            }
            // Pattern keywords that can be used as named argument names
            TokenKind::Map => {
                self.advance();
                Ok(self.interner.intern("map"))
            }
            TokenKind::Filter => {
                self.advance();
                Ok(self.interner.intern("filter"))
            }
            TokenKind::Find => {
                self.advance();
                Ok(self.interner.intern("find"))
            }
            TokenKind::Parallel => {
                self.advance();
                Ok(self.interner.intern("parallel"))
            }
            TokenKind::Timeout => {
                self.advance();
                Ok(self.interner.intern("timeout"))
            }
            _ => Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1004,
                format!("expected identifier or keyword, found {:?}", self.current_kind()),
                self.current_span(),
            )),
        }
    }

    fn recover_to_function(&mut self) {
        while !self.is_at_end() {
            if self.check(TokenKind::At) {
                return;
            }
            self.advance();
        }
    }

    fn match_equality_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::EqEq => Some(BinaryOp::Eq),
            TokenKind::NotEq => Some(BinaryOp::NotEq),
            _ => None,
        }
    }

    fn match_comparison_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Lt => Some(BinaryOp::Lt),
            TokenKind::LtEq => Some(BinaryOp::LtEq),
            TokenKind::Gt => Some(BinaryOp::Gt),
            TokenKind::GtEq => Some(BinaryOp::GtEq),
            _ => None,
        }
    }

    fn match_shift_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Shl => Some(BinaryOp::Shl),
            TokenKind::Shr => Some(BinaryOp::Shr),
            _ => None,
        }
    }

    fn match_additive_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        }
    }

    fn match_multiplicative_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            TokenKind::Percent => Some(BinaryOp::Mod),
            _ => None,
        }
    }

    fn match_unary_op(&self) -> Option<UnaryOp> {
        match self.current_kind() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        }
    }

    /// Match function_seq keywords. Returns Some(true) for try, Some(false) for run.
    fn match_function_seq_kind(&self) -> Option<bool> {
        match self.current_kind() {
            TokenKind::Run => Some(false),
            TokenKind::Try => Some(true),
            _ => None,
        }
    }

    /// Match function_exp keywords.
    fn match_function_exp_kind(&self) -> Option<FunctionExpKind> {
        // Pattern keywords are always keywords (map, filter, fold, etc.)
        match self.current_kind() {
            TokenKind::Map => return Some(FunctionExpKind::Map),
            TokenKind::Filter => return Some(FunctionExpKind::Filter),
            TokenKind::Fold => return Some(FunctionExpKind::Fold),
            TokenKind::Find => return Some(FunctionExpKind::Find),
            TokenKind::Collect => return Some(FunctionExpKind::Collect),
            TokenKind::Recurse => return Some(FunctionExpKind::Recurse),
            TokenKind::Parallel => return Some(FunctionExpKind::Parallel),
            TokenKind::Spawn => return Some(FunctionExpKind::Spawn),
            TokenKind::Timeout => return Some(FunctionExpKind::Timeout),
            TokenKind::Retry => return Some(FunctionExpKind::Retry),
            TokenKind::Cache => return Some(FunctionExpKind::Cache),
            TokenKind::Validate => return Some(FunctionExpKind::Validate),
            TokenKind::With => return Some(FunctionExpKind::With),
            _ => {}
        }

        // Built-in functions are context-sensitive: only keywords when followed by `(`
        // This allows `let len = 5` while still supporting `len(.collection: x)`
        if !self.next_is_lparen() {
            return None;
        }

        match self.current_kind() {
            TokenKind::Assert => Some(FunctionExpKind::Assert),
            TokenKind::AssertEq => Some(FunctionExpKind::AssertEq),
            TokenKind::AssertNe => Some(FunctionExpKind::AssertNe),
            TokenKind::Len => Some(FunctionExpKind::Len),
            TokenKind::IsEmpty => Some(FunctionExpKind::IsEmpty),
            TokenKind::IsSome => Some(FunctionExpKind::IsSome),
            TokenKind::IsNone => Some(FunctionExpKind::IsNone),
            TokenKind::IsOk => Some(FunctionExpKind::IsOk),
            TokenKind::IsErr => Some(FunctionExpKind::IsErr),
            TokenKind::Print => Some(FunctionExpKind::Print),
            TokenKind::Panic => Some(FunctionExpKind::Panic),
            TokenKind::Compare => Some(FunctionExpKind::Compare),
            TokenKind::Min => Some(FunctionExpKind::Min),
            TokenKind::Max => Some(FunctionExpKind::Max),
            _ => None,
        }
    }
}

/// Parse result containing module, arena, and any errors.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseResult {
    pub module: Module,
    pub arena: ExprArena,
    pub errors: Vec<ParseError>,
}

impl ParseResult {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Parse error with error code for rich diagnostics.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseError {
    /// Error code for searchability.
    pub code: crate::diagnostic::ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Location of the error.
    pub span: Span,
    /// Optional context for suggestions.
    pub context: Option<String>,
}

impl ParseError {
    /// Create a new parse error.
    pub fn new(code: crate::diagnostic::ErrorCode, message: impl Into<String>, span: Span) -> Self {
        ParseError {
            code,
            message: message.into(),
            span,
            context: None,
        }
    }

    /// Add context for better error messages.
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Convert to a full Diagnostic for rich error reporting.
    pub fn to_diagnostic(&self) -> crate::diagnostic::Diagnostic {
        crate::diagnostic::Diagnostic::error(self.code)
            .with_message(&self.message)
            .with_label(self.span, self.context.as_deref().unwrap_or("here"))
    }
}

/// Parse tokens into a module.
pub fn parse(tokens: &TokenList, interner: &StringInterner) -> ParseResult {
    let parser = Parser::new(tokens, interner);
    parser.parse_module()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    fn parse_source(source: &str) -> ParseResult {
        let interner = StringInterner::new();
        let tokens = lexer::lex(source, &interner);
        parse(&tokens, &interner)
    }

    #[test]
    fn test_parse_literal() {
        let result = parse_source("@main () -> int = 42");

        assert!(!result.has_errors());
        assert_eq!(result.module.functions.len(), 1);

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);
        assert!(matches!(body.kind, ExprKind::Int(42)));
    }

    #[test]
    fn test_parse_binary_expr() {
        let result = parse_source("@add () -> int = 1 + 2 * 3");

        assert!(!result.has_errors());

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        // Should be Add(1, Mul(2, 3)) due to precedence
        if let ExprKind::Binary { op: BinaryOp::Add, left, right } = &body.kind {
            assert!(matches!(result.arena.get_expr(*left).kind, ExprKind::Int(1)));

            let right_expr = result.arena.get_expr(*right);
            assert!(matches!(right_expr.kind, ExprKind::Binary { op: BinaryOp::Mul, .. }));
        } else {
            panic!("Expected binary add expression");
        }
    }

    #[test]
    fn test_parse_if_expr() {
        let result = parse_source("@test () -> int = if true then 1 else 2");

        assert!(!result.has_errors());

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::If { cond, then_branch, else_branch } = &body.kind {
            assert!(matches!(result.arena.get_expr(*cond).kind, ExprKind::Bool(true)));
            assert!(matches!(result.arena.get_expr(*then_branch).kind, ExprKind::Int(1)));
            assert!(else_branch.is_some());
        } else {
            panic!("Expected if expression");
        }
    }

    #[test]
    fn test_parse_function_seq_run() {
        let result = parse_source("@test () -> int = run(let x = 1, let y = 2, x + y)");

        if result.has_errors() {
            eprintln!("Parse errors: {:?}", result.errors);
        }
        assert!(!result.has_errors());

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::FunctionSeq(FunctionSeq::Run { bindings, .. }) = &body.kind {
            let seq_bindings = result.arena.get_seq_bindings(*bindings);
            assert_eq!(seq_bindings.len(), 2);
        } else {
            panic!("Expected run function_seq, got {:?}", body.kind);
        }
    }

    #[test]
    fn test_parse_let_expression() {
        let result = parse_source("@test () = let x = 1");

        if result.has_errors() {
            eprintln!("Parse errors: {:?}", result.errors);
        }
        assert!(!result.has_errors(), "Expected no parse errors");

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::Let { pattern, ty, mutable, .. } = &body.kind {
            assert!(matches!(pattern, BindingPattern::Name(_)));
            assert!(ty.is_none());
            assert!(!mutable);
        } else {
            panic!("Expected let expression, got {:?}", body.kind);
        }
    }

    #[test]
    fn test_parse_let_with_type() {
        let result = parse_source("@test () = let x: int = 1");

        if result.has_errors() {
            eprintln!("Parse errors: {:?}", result.errors);
        }
        assert!(!result.has_errors());

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::Let { ty, .. } = &body.kind {
            assert!(ty.is_some());
        } else {
            panic!("Expected let expression");
        }
    }

    #[test]
    fn test_parse_run_with_let() {
        let result = parse_source("@test () = run(let x = 1, x)");

        if result.has_errors() {
            eprintln!("Parse errors: {:?}", result.errors);
        }
        assert!(!result.has_errors());

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::FunctionSeq(FunctionSeq::Run { bindings, .. }) = &body.kind {
            let seq_bindings = result.arena.get_seq_bindings(*bindings);
            assert_eq!(seq_bindings.len(), 1);
        } else {
            panic!("Expected run function_seq, got {:?}", body.kind);
        }
    }

    #[test]
    fn test_parse_function_exp_map() {
        let result = parse_source("@test () = map(.over: [1, 2], .transform: (x) -> x)");

        if result.has_errors() {
            eprintln!("Parse errors: {:?}", result.errors);
        }
        assert!(!result.has_errors(), "Expected no parse errors");

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::FunctionExp(func_exp) = &body.kind {
            assert!(matches!(func_exp.kind, FunctionExpKind::Map));
            let props = result.arena.get_named_exprs(func_exp.props);
            assert_eq!(props.len(), 2);
        } else {
            panic!("Expected map function_exp, got {:?}", body.kind);
        }
    }

    #[test]
    fn test_parse_map_multiline() {
        let result = parse_source(r#"@test () = map(
            .over: [1, 2],
            .transform: (x) -> x
        )"#);

        if result.has_errors() {
            eprintln!("Parse errors: {:?}", result.errors);
        }
        assert!(!result.has_errors(), "Expected no parse errors");
    }

    #[test]
    fn test_parse_list() {
        let result = parse_source("@test () -> int = [1, 2, 3]");

        assert!(!result.has_errors());

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::List(range) = &body.kind {
            assert_eq!(range.len(), 3);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_parse_result_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        let result1 = parse_source("@main () -> int = 42");
        let result2 = parse_source("@main () -> int = 42");
        let result3 = parse_source("@main () -> int = 43");

        set.insert(result1);
        set.insert(result2); // duplicate
        set.insert(result3);

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_parse_collect_pattern() {
        let result = parse_source(r#"@main () = collect(
            .range: 1..4,
            .transform: (x: int) -> x * x
        )"#);

        for err in &result.errors {
            eprintln!("Parse error: {:?}", err);
        }
        assert!(result.errors.is_empty(), "Unexpected parse errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_runner_syntax() {
        // Test the exact syntax used in the runner tests
        // Functions are called without @ prefix
        let result = parse_source(r#"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(.left: add(.a: 1, .b: 2), .right: 3)
)
"#);

        for err in &result.errors {
            eprintln!("Parse error: {:?}", err);
        }
        assert!(result.errors.is_empty(), "Unexpected parse errors: {:?}", result.errors);
        assert_eq!(result.module.functions.len(), 1, "Expected 1 function");
        assert_eq!(result.module.tests.len(), 1, "Expected 1 test");
    }

    #[test]
    fn test_at_in_expression_is_error() {
        // @ is only for function definitions, not calls
        // Using @name(...) in an expression should be a syntax error
        let result = parse_source(r#"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    @add(.a: 1, .b: 2)
)
"#);

        assert!(result.has_errors(), "Expected parse error for @add in expression");
    }
}
