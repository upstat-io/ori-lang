//! Recursive descent parser for Sigil.
//!
//! Produces flat AST using ExprArena.

mod cursor;
mod grammar;
mod recovery;

pub use cursor::Cursor;
pub use recovery::{RecoverySet, synchronize};

use crate::ir::{
    ExprArena, Function, Module, Name, Span, StringInterner,
    TestDef, Token, TokenKind, TokenList,
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
}

impl<'a> Parser<'a> {
    /// Create a new parser.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        Parser {
            cursor: Cursor::new(tokens, interner),
            arena: ExprArena::new(),
        }
    }

    // -------------------------------------------------------------------------
    // Cursor delegation methods
    // These delegate to the underlying Cursor for token navigation.
    // -------------------------------------------------------------------------

    #[inline]
    fn current(&self) -> &Token {
        self.cursor.current()
    }

    #[inline]
    fn current_kind(&self) -> TokenKind {
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
    fn check(&self, kind: TokenKind) -> bool {
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
    fn soft_keyword_to_name(&self) -> Option<&'static str> {
        self.cursor.soft_keyword_to_name()
    }

    #[inline]
    fn advance(&mut self) -> &Token {
        self.cursor.advance()
    }

    #[inline]
    fn skip_newlines(&mut self) {
        self.cursor.skip_newlines()
    }

    #[inline]
    fn expect(&mut self, kind: TokenKind) -> Result<&Token, ParseError> {
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

    /// Recovery: skip to next statement (@ or use or EOF)
    fn recover_to_next_statement(&mut self) {
        recovery::synchronize(&mut self.cursor, RecoverySet::STMT_BOUNDARY);
    }

    fn recover_to_function(&mut self) {
        recovery::synchronize(&mut self.cursor, RecoverySet::FUNCTION_BOUNDARY);
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
    use crate::ir::{BinaryOp, BindingPattern, ExprKind, FunctionExpKind, FunctionSeq};
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
        let result = parse_source(r#"@test () = timeout(
            operation: print(msg: "hi"),
            after: 5s
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
    fn test_parse_timeout_pattern() {
        let result = parse_source(r#"@main () = timeout(
            operation: print(msg: "hello"),
            after: 5s
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
    let result = add(a: 1, b: 2),
    print(msg: "done")
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
    @add(a: 1, b: 2)
)
"#);

        assert!(result.has_errors(), "Expected parse error for @add in expression");
    }
}
