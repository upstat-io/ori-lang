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
mod tests;

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
            } else if self.check(&TokenKind::Def)
                && matches!(self.peek_next_kind(), TokenKind::Impl)
            {
                // def impl TraitName { ... }
                let result = self.parse_def_impl_with_progress(visibility);
                self.handle_parse_result(
                    result,
                    &mut module.def_impls,
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
