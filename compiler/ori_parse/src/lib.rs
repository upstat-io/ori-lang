//! Recursive descent parser for Ori.
//!
//! Produces flat AST using `ExprArena`.

mod context;
mod cursor;
mod error;
mod grammar;
mod progress;
mod recovery;

#[cfg(test)]
mod compositional_tests;

pub use context::ParseContext;
pub use cursor::Cursor;
pub use error::ParseError;
pub use progress::{ParseResult, Progress, WithProgress};
pub use recovery::{synchronize, RecoverySet};

use ori_ir::{
    ExprArena, Function, Module, Name, Span, StringInterner, TestDef, Token, TokenKind, TokenList,
    Visibility,
};

/// Result of parsing a definition starting with @.
/// Can be either a function or a test.
enum FunctionOrTest {
    Function(Function),
    Test(TestDef),
}

// Re-export ParsedAttrs from grammar module.
pub(crate) use grammar::ParsedAttrs;

/// Parser state.
pub struct Parser<'a> {
    cursor: Cursor<'a>,
    arena: ExprArena,
    /// Current parsing context flags.
    pub(crate) context: ParseContext,
}

impl<'a> Parser<'a> {
    /// Create a new parser.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        Parser {
            cursor: Cursor::new(tokens, interner),
            arena: ExprArena::new(),
            context: ParseContext::new(),
        }
    }

    /// Take ownership of the arena, replacing it with an empty one.
    ///
    /// This is useful for tests that need to access the arena after parsing.
    #[cfg(test)]
    pub fn take_arena(&mut self) -> ExprArena {
        std::mem::take(&mut self.arena)
    }

    // --- Context Management ---
    //
    // These methods support context-sensitive parsing. Some are not yet used
    // internally but are part of the public API for parser extensions and testing.

    /// Get the current parsing context.
    #[inline]
    #[allow(dead_code)] // Used in tests and future parser extensions
    pub(crate) fn context(&self) -> ParseContext {
        self.context
    }

    /// Execute a closure with additional context flags, then restore the original context.
    ///
    /// This is the primary way to temporarily modify parsing context.
    ///
    /// # Example
    /// ```ignore
    /// // Parse condition without allowing struct literals
    /// let cond = self.with_context(ParseContext::NO_STRUCT_LIT, |p| {
    ///     p.parse_expr()
    /// })?;
    /// ```
    #[inline]
    pub(crate) fn with_context<T, F>(&mut self, add: ParseContext, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let old = self.context;
        self.context = self.context.with(add);
        let result = f(self);
        self.context = old;
        result
    }

    /// Execute a closure with context flags removed, then restore the original context.
    ///
    /// # Example
    /// ```ignore
    /// // Parse body allowing struct literals again
    /// let body = self.without_context(ParseContext::NO_STRUCT_LIT, |p| {
    ///     p.parse_expr()
    /// })?;
    /// ```
    #[inline]
    #[allow(dead_code)] // Used in tests and future parser extensions
    pub(crate) fn without_context<T, F>(&mut self, remove: ParseContext, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let old = self.context;
        self.context = self.context.without(remove);
        let result = f(self);
        self.context = old;
        result
    }

    /// Check if a context flag is set.
    #[inline]
    #[allow(dead_code)] // Used in tests and future parser extensions
    pub(crate) fn has_context(&self, flag: ParseContext) -> bool {
        self.context.has(flag)
    }

    /// Check if struct literals are allowed in the current context.
    #[inline]
    pub(crate) fn allows_struct_lit(&self) -> bool {
        self.context.allows_struct_lit()
    }

    /// Cursor delegation methods - delegate to the underlying Cursor for token navigation.
    #[inline]
    fn current(&self) -> &Token {
        self.cursor.current()
    }

    #[inline]
    fn current_kind(&self) -> &TokenKind {
        self.cursor.current_kind()
    }

    #[inline]
    fn current_span(&self) -> Span {
        self.cursor.current_span()
    }

    #[inline]
    fn previous_span(&self) -> Span {
        self.cursor.previous_span()
    }

    #[inline]
    fn is_at_end(&self) -> bool {
        self.cursor.is_at_end()
    }

    #[inline]
    fn check(&self, kind: &TokenKind) -> bool {
        self.cursor.check(kind)
    }

    #[inline]
    fn check_ident(&self) -> bool {
        self.cursor.check_ident()
    }

    #[inline]
    fn check_type_keyword(&self) -> bool {
        self.cursor.check_type_keyword()
    }

    #[inline]
    fn peek_next_kind(&self) -> &TokenKind {
        self.cursor.peek_next_kind()
    }

    #[inline]
    fn next_is_lparen(&self) -> bool {
        self.cursor.next_is_lparen()
    }

    #[inline]
    fn next_is_colon(&self) -> bool {
        self.cursor.next_is_colon()
    }

    #[inline]
    fn is_named_arg_start(&self) -> bool {
        self.cursor.is_named_arg_start()
    }

    #[inline]
    fn is_with_capability_syntax(&self) -> bool {
        self.cursor.is_with_capability_syntax()
    }

    #[inline]
    fn soft_keyword_to_name(&self) -> Option<&'static str> {
        self.cursor.soft_keyword_to_name()
    }

    /// Check if looking at `>` followed immediately by `>` (no whitespace).
    /// Used for detecting `>>` shift operator in expression context.
    #[inline]
    fn is_shift_right(&self) -> bool {
        self.cursor.is_shift_right()
    }

    /// Check if looking at `>` followed immediately by `=` (no whitespace).
    /// Used for detecting `>=` comparison operator in expression context.
    #[inline]
    fn is_greater_equal(&self) -> bool {
        self.cursor.is_greater_equal()
    }

    #[inline]
    fn advance(&mut self) -> &Token {
        self.cursor.advance()
    }

    #[inline]
    fn skip_newlines(&mut self) {
        self.cursor.skip_newlines();
    }

    #[inline]
    fn expect(&mut self, kind: &TokenKind) -> Result<&Token, ParseError> {
        self.cursor.expect(kind)
    }

    #[inline]
    fn expect_ident(&mut self) -> Result<Name, ParseError> {
        self.cursor.expect_ident()
    }

    #[inline]
    fn expect_ident_or_keyword(&mut self) -> Result<Name, ParseError> {
        self.cursor.expect_ident_or_keyword()
    }

    /// Get access to the string interner.
    #[inline]
    fn interner(&self) -> &StringInterner {
        self.cursor.interner()
    }

    /// Get the current position in the token stream.
    ///
    /// Used for progress tracking - compare positions before and after
    /// parsing to determine if tokens were consumed.
    #[inline]
    pub(crate) fn position(&self) -> usize {
        self.cursor.position()
    }

    /// Determine progress based on position change.
    ///
    /// Returns `Progress::Made` if the current position is greater than
    /// the saved position, otherwise `Progress::None`.
    #[inline]
    pub(crate) fn progress_since(&self, saved_pos: usize) -> Progress {
        if self.position() > saved_pos {
            Progress::Made
        } else {
            Progress::None
        }
    }

    /// Execute a parse function and track progress automatically.
    ///
    /// Returns a `ParseResult` with progress determined by whether tokens were consumed.
    #[inline]
    #[allow(dead_code)] // Will be used as parsing methods are converted
    pub(crate) fn with_progress<T, F>(&mut self, f: F) -> ParseResult<T>
    where
        F: FnOnce(&mut Self) -> Result<T, ParseError>,
    {
        let start_pos = self.position();
        let result = f(self);
        let progress = self.progress_since(start_pos);
        ParseResult { progress, result }
    }

    /// Handle a parse result by pushing to a collection on success, or recording error and recovering.
    ///
    /// This is a helper for the common pattern in module parsing:
    /// 1. Parse an item with progress tracking
    /// 2. On success: push to collection
    /// 3. On error: if progress was made, recover; then record error
    fn handle_parse_result<T>(
        &mut self,
        result: ParseResult<T>,
        collection: &mut Vec<T>,
        errors: &mut Vec<ParseError>,
        recover: impl FnOnce(&mut Self),
    ) {
        let made_progress = result.made_progress();
        match result.into_result() {
            Ok(item) => collection.push(item),
            Err(e) => {
                if made_progress {
                    recover(self);
                }
                errors.push(e);
            }
        }
    }

    /// Parse a module (collection of function definitions and tests).
    ///
    /// Uses progress-aware parsing for improved error recovery:
    /// - If parsing fails without progress (no tokens consumed), we skip unknown tokens
    /// - If parsing fails with progress (tokens consumed), we synchronize to a recovery point
    pub fn parse_module(mut self) -> ParseOutput {
        let mut module = Module::new();
        let mut errors = Vec::new();

        // Parse imports first (must appear at beginning per spec)
        // Includes both regular imports and public re-exports
        while !self.is_at_end() {
            self.skip_newlines();
            if self.is_at_end() {
                break;
            }

            // Check for pub use (re-export)
            let is_pub_use =
                self.check(&TokenKind::Pub) && matches!(self.peek_next_kind(), TokenKind::Use);

            if self.check(&TokenKind::Use) || is_pub_use {
                let visibility = if is_pub_use {
                    self.advance(); // consume 'pub'
                    Visibility::Public
                } else {
                    Visibility::Private
                };
                let result = self.with_progress(|p| p.parse_use_inner(visibility));
                self.handle_parse_result(
                    result,
                    &mut module.imports,
                    &mut errors,
                    Self::recover_to_next_statement,
                );
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

            // Check for pub modifier
            let visibility = if self.check(&TokenKind::Pub) {
                self.advance();
                Visibility::Public
            } else {
                Visibility::Private
            };

            if self.check(&TokenKind::At) {
                let result = self.parse_function_or_test_with_progress(attrs, visibility);
                let made_progress = result.made_progress();
                match result.into_result() {
                    Ok(FunctionOrTest::Function(func)) => module.functions.push(func),
                    Ok(FunctionOrTest::Test(test)) => module.tests.push(test),
                    Err(e) => {
                        // Progress-aware recovery: only synchronize if we consumed tokens
                        if made_progress {
                            self.recover_to_function();
                        }
                        errors.push(e);
                    }
                }
            } else if self.check(&TokenKind::Trait) {
                let result = self.parse_trait_with_progress(visibility);
                self.handle_parse_result(
                    result,
                    &mut module.traits,
                    &mut errors,
                    Self::recover_to_function,
                );
            } else if self.check(&TokenKind::Impl) {
                let result = self.parse_impl_with_progress();
                self.handle_parse_result(
                    result,
                    &mut module.impls,
                    &mut errors,
                    Self::recover_to_function,
                );
            } else if self.check(&TokenKind::Extend) {
                let result = self.parse_extend_with_progress();
                self.handle_parse_result(
                    result,
                    &mut module.extends,
                    &mut errors,
                    Self::recover_to_function,
                );
            } else if self.check(&TokenKind::Type) {
                let result = self.parse_type_decl_with_progress(attrs, visibility);
                self.handle_parse_result(
                    result,
                    &mut module.types,
                    &mut errors,
                    Self::recover_to_function,
                );
            } else if self.check(&TokenKind::Dollar) {
                let result = self.parse_config_with_progress(visibility);
                self.handle_parse_result(
                    result,
                    &mut module.configs,
                    &mut errors,
                    Self::recover_to_function,
                );
            } else if self.check(&TokenKind::Use) {
                // Import after declarations - error
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "import statements must appear at the beginning of the file".to_string(),
                    self.current_span(),
                ));
                // Skip the entire use statement to avoid infinite loop
                // (recover_to_next_statement would stop at this same Use token)
                self.advance(); // skip 'use'
                while !self.is_at_end()
                    && !self.check(&TokenKind::At)
                    && !self.check(&TokenKind::Trait)
                    && !self.check(&TokenKind::Impl)
                    && !self.check(&TokenKind::Type)
                    && !self.check(&TokenKind::Use)
                {
                    self.advance();
                }
            } else if !attrs.is_empty() {
                // Attributes without a following function/test
                errors.push(ParseError {
                    code: ori_diagnostic::ErrorCode::E1006,
                    message: "attributes must be followed by a function or test definition"
                        .to_string(),
                    span: self.current_span(),
                    context: None,
                });
                self.advance();
            } else {
                // Skip unknown token
                self.advance();
            }
        }

        ParseOutput {
            module,
            arena: self.arena,
            errors,
        }
    }

    /// Recovery: skip to next statement (@ or use or EOF)
    fn recover_to_next_statement(&mut self) {
        recovery::synchronize(&mut self.cursor, RecoverySet::STMT_BOUNDARY);
    }

    fn recover_to_function(&mut self) {
        recovery::synchronize(&mut self.cursor, RecoverySet::FUNCTION_BOUNDARY);
    }
}

/// Output from parsing a module, containing the module, arena, and any errors.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseOutput {
    pub module: Module,
    pub arena: ExprArena,
    pub errors: Vec<ParseError>,
}

impl ParseOutput {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Parse tokens into a module.
pub fn parse(tokens: &TokenList, interner: &StringInterner) -> ParseOutput {
    let parser = Parser::new(tokens, interner);
    parser.parse_module()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::{BinaryOp, BindingPattern, ExprKind, FunctionExpKind, FunctionSeq};

    fn parse_source(source: &str) -> ParseOutput {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
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
        if let ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = &body.kind
        {
            assert!(matches!(
                result.arena.get_expr(*left).kind,
                ExprKind::Int(1)
            ));

            let right_expr = result.arena.get_expr(*right);
            assert!(matches!(
                right_expr.kind,
                ExprKind::Binary {
                    op: BinaryOp::Mul,
                    ..
                }
            ));
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

        if let ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } = &body.kind
        {
            assert!(matches!(
                result.arena.get_expr(*cond).kind,
                ExprKind::Bool(true)
            ));
            assert!(matches!(
                result.arena.get_expr(*then_branch).kind,
                ExprKind::Int(1)
            ));
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

        if let ExprKind::Let {
            pattern,
            ty,
            mutable,
            ..
        } = &body.kind
        {
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
    fn test_parse_function_exp_print() {
        // Test parsing print function_exp (one of the remaining compiler patterns)
        let result = parse_source("@test () = print(msg: \"hello\")");

        if result.has_errors() {
            eprintln!("Parse errors: {:?}", result.errors);
        }
        assert!(!result.has_errors(), "Expected no parse errors");

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::FunctionExp(func_exp) = &body.kind {
            assert!(matches!(func_exp.kind, FunctionExpKind::Print));
            let props = result.arena.get_named_exprs(func_exp.props);
            assert_eq!(props.len(), 1);
        } else {
            panic!("Expected print function_exp, got {:?}", body.kind);
        }
    }

    #[test]
    fn test_parse_timeout_multiline() {
        // Test parsing timeout function_exp with multiline format
        let result = parse_source(
            r#"@test () = timeout(
            operation: print(msg: "hi"),
            after: 5s
        )"#,
        );

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
    fn test_parse_timeout_pattern() {
        let result = parse_source(
            r#"@main () = timeout(
            operation: print(msg: "hello"),
            after: 5s
        )"#,
        );

        for err in &result.errors {
            eprintln!("Parse error: {err:?}");
        }
        assert!(
            result.errors.is_empty(),
            "Unexpected parse errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_parse_runner_syntax() {
        // Test the exact syntax used in the runner tests
        // Functions are called without @ prefix
        let result = parse_source(
            r#"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    let result = add(a: 1, b: 2),
    print(msg: "done")
)
"#,
        );

        for err in &result.errors {
            eprintln!("Parse error: {err:?}");
        }
        assert!(
            result.errors.is_empty(),
            "Unexpected parse errors: {:?}",
            result.errors
        );
        assert_eq!(result.module.functions.len(), 1, "Expected 1 function");
        assert_eq!(result.module.tests.len(), 1, "Expected 1 test");
    }

    #[test]
    fn test_at_in_expression_is_error() {
        // @ is only for function definitions, not calls
        // Using @name(...) in an expression should be a syntax error
        let result = parse_source(
            r"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    @add(a: 1, b: 2)
)
",
        );

        assert!(
            result.has_errors(),
            "Expected parse error for @add in expression"
        );
    }

    #[test]
    fn test_uses_clause_single_capability() {
        let result = parse_source(
            r"
@fetch (url: str) -> str uses Http = Http.get(url: url)
",
        );

        assert!(!result.has_errors(), "Expected no parse errors");
        assert_eq!(result.module.functions.len(), 1);

        let func = &result.module.functions[0];
        assert_eq!(func.capabilities.len(), 1);
    }

    #[test]
    fn test_uses_clause_multiple_capabilities() {
        let result = parse_source(
            r#"
@save (data: str) -> void uses FileSystem, Async = FileSystem.write(path: "/data", content: data)
"#,
        );

        assert!(!result.has_errors(), "Expected no parse errors");
        assert_eq!(result.module.functions.len(), 1);

        let func = &result.module.functions[0];
        assert_eq!(func.capabilities.len(), 2);
    }

    #[test]
    fn test_uses_clause_with_where() {
        // uses clause must come before where clause
        let result = parse_source(
            r"
@process<T> (data: T) -> T uses Logger where T: Clone = data
",
        );

        assert!(!result.has_errors(), "Expected no parse errors");
        assert_eq!(result.module.functions.len(), 1);

        let func = &result.module.functions[0];
        assert_eq!(func.capabilities.len(), 1);
        assert_eq!(func.where_clauses.len(), 1);
    }

    #[test]
    fn test_no_uses_clause() {
        // Pure function - no uses clause
        let result = parse_source(
            r"
@add (a: int, b: int) -> int = a + b
",
        );

        assert!(!result.has_errors(), "Expected no parse errors");
        assert_eq!(result.module.functions.len(), 1);

        let func = &result.module.functions[0];
        assert!(func.capabilities.is_empty());
    }

    #[test]
    fn test_with_capability_expression() {
        // with Capability = Provider in body
        let result = parse_source(
            r"
@example () -> int =
    with Http = MockHttp in
        42
",
        );

        assert!(
            !result.has_errors(),
            "Expected no parse errors: {:?}",
            result.errors
        );
        assert_eq!(result.module.functions.len(), 1);

        // Find the WithCapability expression in the body
        let func = &result.module.functions[0];
        let body_expr = result.arena.get_expr(func.body);
        assert!(
            matches!(body_expr.kind, ExprKind::WithCapability { .. }),
            "Expected WithCapability, got {:?}",
            body_expr.kind
        );
    }

    #[test]
    fn test_with_capability_with_struct_provider() {
        // with Capability = StructLiteral { field: value } in body
        let result = parse_source(
            r#"
@example () -> int =
    with Http = RealHttp { base_url: "https://api.example.com" } in
        fetch(url: "/data")
"#,
        );

        assert!(
            !result.has_errors(),
            "Expected no parse errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_with_capability_nested() {
        // Nested capability provisions
        let result = parse_source(
            r"
@example () -> int =
    with Http = MockHttp in
        with Cache = MockCache in
            42
",
        );

        assert!(
            !result.has_errors(),
            "Expected no parse errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_no_async_type_modifier() {
        // Ori does not support `async` as a type modifier.
        // Instead, use `uses Async` capability.
        // The `async` keyword is reserved but should cause a parse error when used as type.
        let result = parse_source(
            r"
@example () -> async int = 42
",
        );

        // Should have parse error - async is not a valid type modifier
        assert!(
            result.has_errors(),
            "async type modifier should not be supported"
        );
    }

    #[test]
    fn test_async_keyword_reserved() {
        // The async keyword is reserved and cannot be used as an identifier
        let result = parse_source(
            r"
@test () -> int = run(
    let async = 42,
    async,
)
",
        );

        // Should have parse error - async is a reserved keyword
        assert!(result.has_errors(), "async should be a reserved keyword");
    }

    #[test]
    fn test_uses_async_capability_parses() {
        // The correct way to declare async behavior: uses Async capability
        let result = parse_source(
            r"
trait Async {}

@async_op () -> int uses Async = 42
",
        );

        assert!(
            !result.has_errors(),
            "uses Async should parse correctly: {:?}",
            result.errors
        );

        // Verify the function has the Async capability
        let func = &result.module.functions[0];
        assert_eq!(func.capabilities.len(), 1);
    }

    #[test]
    fn test_shift_right_operator() {
        // >> is detected as two adjacent > tokens in expression context
        let result = parse_source("@test () -> int = 8 >> 2");

        assert!(
            !result.has_errors(),
            "Expected no parse errors: {:?}",
            result.errors
        );

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::Binary {
            op: BinaryOp::Shr, ..
        } = &body.kind
        {
            // Success
        } else {
            panic!(
                "Expected right shift (>>) binary expression, got {:?}",
                body.kind
            );
        }
    }

    #[test]
    fn test_greater_equal_operator() {
        // >= is detected as adjacent > and = tokens in expression context
        let result = parse_source("@test () -> bool = 5 >= 3");

        assert!(
            !result.has_errors(),
            "Expected no parse errors: {:?}",
            result.errors
        );

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::Binary {
            op: BinaryOp::GtEq, ..
        } = &body.kind
        {
            // Success
        } else {
            panic!(
                "Expected greater-equal (>=) binary expression, got {:?}",
                body.kind
            );
        }
    }

    #[test]
    fn test_shift_left_operator() {
        // << should still work (single token from lexer)
        let result = parse_source("@test () -> int = 2 << 3");

        assert!(
            !result.has_errors(),
            "Expected no parse errors: {:?}",
            result.errors
        );

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::Binary {
            op: BinaryOp::Shl, ..
        } = &body.kind
        {
            // Success
        } else {
            panic!(
                "Expected left shift (<<) binary expression, got {:?}",
                body.kind
            );
        }
    }

    #[test]
    fn test_greater_than_operator() {
        // Single > should still work
        let result = parse_source("@test () -> bool = 5 > 3");

        assert!(
            !result.has_errors(),
            "Expected no parse errors: {:?}",
            result.errors
        );

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::Binary {
            op: BinaryOp::Gt, ..
        } = &body.kind
        {
            // Success
        } else {
            panic!(
                "Expected greater-than (>) binary expression, got {:?}",
                body.kind
            );
        }
    }

    #[test]
    fn test_shift_right_with_space() {
        // > > with space should NOT be treated as >>
        let result = parse_source("@test () -> int = 8 > > 2");

        // This should have errors because `> > 2` is invalid syntax
        // (comparison followed by another >)
        assert!(
            result.has_errors(),
            "Expected parse errors for `> > 2` with space"
        );
    }

    #[test]
    fn test_greater_equal_with_space() {
        // > = with space should NOT be treated as >=
        let result = parse_source("@test () -> bool = 5 > = 3");

        // This should have errors because `> = 3` is invalid syntax
        assert!(
            result.has_errors(),
            "Expected parse errors for `> = 3` with space"
        );
    }

    #[test]
    fn test_nested_generic_and_shift() {
        // Test that nested generics work in a type annotation and >> works in expression
        let result = parse_source(
            r"
@test () -> Result<Result<int, str>, str> = run(
    let x = 8 >> 2,
    Ok(Ok(x))
)",
        );

        assert!(
            !result.has_errors(),
            "Expected no parse errors for nested generics and >> operator: {:?}",
            result.errors
        );
    }

    // --- Context Management Tests ---

    #[test]
    fn test_struct_literal_in_expression() {
        // Struct literals work normally in expressions
        let result = parse_source(
            r"
type Point = { x: int, y: int }

@test () -> int = Point { x: 1, y: 2 }.x
",
        );

        assert!(
            !result.has_errors(),
            "Struct literal should parse in normal expression: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_struct_literal_in_if_then_body() {
        // Struct literals work in the then body of an if expression
        let result = parse_source(
            r"
type Point = { x: int, y: int }

@test () -> int = if true then Point { x: 1, y: 2 }.x else 0
",
        );

        assert!(
            !result.has_errors(),
            "Struct literal should parse in if body: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_if_condition_disallows_struct_literal() {
        // Struct literals are NOT allowed directly in if conditions
        // This is a common pattern in many languages to prevent ambiguity
        // Note: In Ori with `then` keyword, this is mostly for consistency,
        // but it helps prevent confusing code like `if Point { ... }.valid then`
        let result = parse_source(
            r"
type Point = { x: int, y: int }

@test () -> int = if Point { x: 1, y: 2 }.x > 0 then 1 else 0
",
        );

        // This should fail because struct literal is not allowed in if condition
        assert!(
            result.has_errors(),
            "Struct literal should NOT be allowed in if condition"
        );
    }

    #[test]
    fn test_context_methods() {
        // Exercise the context API to ensure it compiles and works
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex("@test () = 42", &interner);
        let mut parser = Parser::new(&tokens, &interner);

        // Test context() getter
        let ctx = parser.context();
        assert_eq!(ctx, ParseContext::NONE);

        // Test has_context()
        assert!(!parser.has_context(ParseContext::IN_LOOP));

        // Test with_context()
        let result = parser.with_context(ParseContext::IN_LOOP, |p| {
            assert!(p.has_context(ParseContext::IN_LOOP));
            42
        });
        assert_eq!(result, 42);
        assert!(!parser.has_context(ParseContext::IN_LOOP)); // restored

        // Test without_context() - first add a context, then remove it
        parser.context = ParseContext::IN_LOOP;
        let result = parser.without_context(ParseContext::IN_LOOP, |p| {
            assert!(!p.has_context(ParseContext::IN_LOOP));
            43
        });
        assert_eq!(result, 43);
        assert!(parser.has_context(ParseContext::IN_LOOP)); // restored
    }
}
