//! Recursive descent parser for Ori.
//!
//! Produces flat AST using `ExprArena`.

mod context;
mod cursor;
mod error;
mod foreign_keywords;
mod grammar;
pub mod incremental;
mod outcome;
mod recovery;
pub mod series;
mod snapshot;

#[cfg(test)]
mod tests;

pub use context::ParseContext;
pub(crate) use cursor::Cursor;
pub use error::{DetachmentReason, ErrorContext, ParseError, ParseWarning};
pub use outcome::ParseOutcome;
pub use recovery::{synchronize, TokenSet, FUNCTION_BOUNDARY, STMT_BOUNDARY};
pub use series::{SeriesConfig, TrailingSeparator};

// Re-export backtracking macros at crate root
// Note: These are defined in outcome.rs and use #[macro_export]
// They're automatically available at crate root via #[macro_export]

use ori_ir::{
    ExprArena, Function, Module, ModuleExtra, Name, SharedArena, Span, StringInterner, TestDef,
    TokenKind, TokenList, Visibility,
};
use tracing::debug;

/// Result of parsing a definition starting with @.
/// Can be either a function or a test.
enum FunctionOrTest {
    Function(Function),
    Test(TestDef),
}

// Re-export ParsedAttrs from grammar module.
pub(crate) use grammar::ParsedAttrs;

/// Pre-interned `Name` values for contextual keywords used in identifier comparisons.
///
/// Avoids acquiring interner read-locks during parsing by comparing `Name` values
/// (u32 equality) instead of looking up strings via `interner().lookup()`.
pub(crate) struct KnownNames {
    // Channel constructors
    pub channel: Name,
    pub channel_in: Name,
    pub channel_out: Name,
    pub channel_all: Name,
    // For-pattern properties
    pub over: Name,
    pub map: Name,
    pub match_: Name,
    pub default: Name,
    // Type syntax
    pub max: Name,
}

impl KnownNames {
    /// Intern all known contextual keywords once.
    fn new(interner: &StringInterner) -> Self {
        Self {
            channel: interner.intern("channel"),
            channel_in: interner.intern("channel_in"),
            channel_out: interner.intern("channel_out"),
            channel_all: interner.intern("channel_all"),
            over: interner.intern("over"),
            map: interner.intern("map"),
            match_: interner.intern("match"),
            default: interner.intern("default"),
            max: interner.intern("max"),
        }
    }
}

/// Parser state.
pub struct Parser<'a> {
    pub(crate) cursor: Cursor<'a>,
    arena: ExprArena,
    /// Current parsing context flags.
    pub(crate) context: ParseContext,
    /// Pre-interned names for contextual keyword comparisons.
    pub(crate) known: KnownNames,
    /// Errors from sub-parsers that lack `&mut Vec<ParseError>` access
    /// (e.g., `parse_type()` detecting reserved syntax like `&T`).
    /// Drained into the main error list in `parse_module()`.
    pub(crate) deferred_errors: Vec<ParseError>,
    /// Warnings from sub-parsers (e.g., unknown calling conventions).
    /// Drained into `ParseOutput.warnings` alongside post-parse warnings.
    pub(crate) deferred_warnings: Vec<ParseWarning>,
}

impl<'a> Parser<'a> {
    /// Create a new parser.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        // Estimate source size for pre-allocation (~5 bytes per token)
        let estimated_source_len = tokens.len() * 5;
        Parser {
            cursor: Cursor::new(tokens, interner),
            arena: ExprArena::with_capacity(estimated_source_len),
            context: ParseContext::new(),
            known: KnownNames::new(interner),
            deferred_errors: Vec::new(),
            deferred_warnings: Vec::new(),
        }
    }

    /// Estimate source size from token count for capacity hints.
    ///
    /// Heuristic: ~5 bytes per token on average.
    #[inline]
    fn estimated_source_len(&self) -> usize {
        self.cursor.token_count() * 5
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
    #[allow(dead_code, reason = "API for tests and future parser extensions")]
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
    #[allow(dead_code, reason = "API for tests and future parser extensions")]
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
    #[allow(dead_code, reason = "API for tests and future parser extensions")]
    pub(crate) fn has_context(&self, flag: ParseContext) -> bool {
        self.context.has(flag)
    }

    /// Check if struct literals are allowed in the current context.
    #[inline]
    pub(crate) fn allows_struct_lit(&self) -> bool {
        self.context.allows_struct_lit()
    }

    // --- Error Context ---
    //
    // These methods support Elm-style error context for better error messages.
    // `ErrorContext` describes what was being parsed when an error occurred,
    // enabling messages like "while parsing an if expression".
    //
    // Note: This is distinct from `ParseContext` (the bitfield for context-sensitive
    // parsing behavior like NO_STRUCT_LIT).

    /// Execute a parser and wrap any hard errors with context.
    ///
    /// This is the Elm-style `in_context` pattern. It:
    /// 1. Runs the provided parser
    /// 2. If it returns `ConsumedErr`, wraps the error with context
    /// 3. Passes through all other outcomes unchanged
    ///
    /// Use this to provide better error messages like "while parsing an if expression".
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
    ///     self.in_error_context(ErrorContext::IfExpression, |p| {
    ///         p.cursor.expect(&TokenKind::If)?;
    ///         let cond = p.parse_expr()?;
    ///         // ...
    ///     })
    /// }
    /// ```
    ///
    /// # Error Messages
    ///
    /// Without context: "expected expression, found `}`"
    /// With context: "expected expression, found `}` (while parsing an if expression)"
    #[inline]
    pub(crate) fn in_error_context<T, F>(
        &mut self,
        context: error::ErrorContext,
        f: F,
    ) -> ParseOutcome<T>
    where
        F: FnOnce(&mut Self) -> ParseOutcome<T>,
    {
        tracing::debug!(context = context.label(), "entering parse context");
        f(self).with_error_context(context)
    }

    /// Attach error context to a `Result`-returning parser function.
    ///
    /// Like [`in_error_context`](Self::in_error_context) but for functions that
    /// return `Result<T, ParseError>` (e.g., postfix operations called via `?`).
    #[inline]
    pub(crate) fn in_error_context_result<T, F>(
        &mut self,
        context: error::ErrorContext,
        f: F,
    ) -> Result<T, ParseError>
    where
        F: FnOnce(&mut Self) -> Result<T, ParseError>,
    {
        tracing::debug!(context = context.label(), "entering parse context");
        f(self).map_err(|mut e| {
            if e.context.is_none() {
                e.context = Some(format!("while parsing {}", context.description()));
            }
            e
        })
    }

    // --- Token Capture ---
    //
    // These methods support lazy token capture for formatters and future macros.
    // Instead of storing tokens directly, we capture index ranges into the
    // cached TokenList, which is very memory efficient.

    /// Execute a parser and capture its tokens.
    ///
    /// This is a convenience method that combines `start_capture()` and
    /// `complete_capture()` with a parsing closure. Use when you always
    /// need to capture tokens.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (expr, capture) = parser.with_capture(|p| p.parse_expr())?;
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "infrastructure for formatters and macro expansion"
    )]
    pub(crate) fn with_capture<T, F>(&mut self, f: F) -> (T, ori_ir::TokenCapture)
    where
        F: FnOnce(&mut Self) -> T,
    {
        let start = self.cursor.start_capture();
        let result = f(self);
        let capture = self.cursor.complete_capture(start);
        (result, capture)
    }

    /// Execute a parser and optionally capture its tokens.
    ///
    /// When `needs_capture` is false, returns `TokenCapture::None` without
    /// the overhead of tracking positions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let needs_tokens = self.context.has(ParseContext::CAPTURE_TOKENS);
    /// let (expr, capture) = parser.capture_if(needs_tokens, |p| p.parse_expr())?;
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "infrastructure for conditional token capture in formatters"
    )]
    pub(crate) fn capture_if<T, F>(
        &mut self,
        needs_capture: bool,
        f: F,
    ) -> (T, ori_ir::TokenCapture)
    where
        F: FnOnce(&mut Self) -> T,
    {
        if needs_capture {
            self.with_capture(f)
        } else {
            (f(self), ori_ir::TokenCapture::None)
        }
    }

    /// Check if the current token matches any kind in the set.
    ///
    /// Unlike `cursor.check()`, this tests against multiple token kinds at once.
    /// Returns `true` if any match is found.
    #[inline]
    #[allow(dead_code, reason = "infrastructure for multi-token error recovery")]
    pub(crate) fn check_one_of(&self, expected: &TokenSet) -> bool {
        expected.contains(self.cursor.current_kind())
    }

    /// Expect one of several token kinds, generating a helpful error if none match.
    ///
    /// Uses `TokenSet::format_expected()` to generate messages like
    /// "expected `,`, `)`, or `}`, found `+`".
    ///
    /// Returns the matched token kind on success.
    #[cold]
    #[allow(
        dead_code,
        reason = "infrastructure for multi-token expect with rich errors"
    )]
    pub(crate) fn expect_one_of(&mut self, expected: &TokenSet) -> Result<TokenKind, ParseError> {
        let current = self.cursor.current_kind();
        if expected.contains(current) {
            let matched = current.clone();
            self.cursor.advance();
            Ok(matched)
        } else {
            Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1001,
                format!(
                    "expected {}, found `{}`",
                    expected.format_expected(),
                    current.display_name()
                ),
                self.cursor.current_span(),
            ))
        }
    }

    // --- Speculative Parsing (Snapshots) ---
    //
    // These methods enable speculative parsing for disambiguation.
    // Use when you need to try a parse, examine the result, and decide
    // whether to keep or discard it.
    //
    // Complements progress tracking:
    // - Progress: simple alternatives (`parse_a().or_else(|| parse_b())`)
    // - Snapshots: complex disambiguation requiring full parse attempts

    /// Create a snapshot of the current parser state.
    ///
    /// The snapshot captures cursor position and context flags. Arena state
    /// is NOT captured—speculative parsing should examine tokens, not allocate.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let snapshot = self.snapshot();
    /// // Try parsing as type
    /// if self.parse_type().is_ok() && self.cursor.check(&TokenKind::Eq) {
    ///     // Commit: this is a type annotation
    /// } else {
    ///     // Rollback and try as expression
    ///     self.restore(snapshot);
    ///     return self.parse_expr();
    /// }
    /// ```
    #[inline]
    pub(crate) fn snapshot(&self) -> snapshot::ParserSnapshot {
        snapshot::ParserSnapshot::new(self.cursor.position(), self.context)
    }

    /// Restore parser state from a snapshot.
    ///
    /// Resets cursor position and context flags to their values when the
    /// snapshot was taken. Does NOT restore arena state.
    #[inline]
    pub(crate) fn restore(&mut self, snapshot: snapshot::ParserSnapshot) {
        self.cursor.set_position(snapshot.cursor_pos);
        self.context = snapshot.context;
    }

    /// Try parsing speculatively, restoring state on failure.
    ///
    /// If the parse function succeeds, returns `Some(result)`.
    /// If it fails, restores parser state and returns `None`.
    ///
    /// This is the primary method for speculative parsing. Use when you
    /// need to try an interpretation and fall back if it doesn't work.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Try parsing as type annotation first
    /// if let Some(ty) = self.try_parse(|p| p.parse_type()) {
    ///     return Ok(TypeOrExpr::Type(ty));
    /// }
    /// // Fall back to expression
    /// let expr = self.parse_expr()?;
    /// Ok(TypeOrExpr::Expr(expr))
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "reserved for grammar disambiguation with backtracking"
    )]
    pub(crate) fn try_parse<T, F>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(&mut Self) -> Result<T, ParseError>,
    {
        let snapshot = self.snapshot();
        if let Ok(result) = f(self) {
            Some(result)
        } else {
            self.restore(snapshot);
            None
        }
    }

    /// Look ahead without side effects.
    ///
    /// Executes the function and then always restores state, returning
    /// whatever the function returned. Use for peeking ahead to make
    /// parsing decisions without consuming tokens.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Check if this looks like a type annotation
    /// let is_type_annotation = self.look_ahead(|p| {
    ///     p.parse_type().is_ok() && p.cursor.check(&TokenKind::Eq)
    /// });
    ///
    /// if is_type_annotation {
    ///     // Parse as type annotation
    /// } else {
    ///     // Parse as expression
    /// }
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "reserved for non-consuming lookahead in grammar disambiguation"
    )]
    pub(crate) fn look_ahead<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let snapshot = self.snapshot();
        let result = f(self);
        self.restore(snapshot);
        result
    }

    /// Handle a `ParseOutcome` by pushing to a collection on success, or recording error and recovering.
    ///
    /// Like `handle_parse_result` but for `ParseOutcome`:
    /// - `ConsumedOk` / `EmptyOk`: push value to collection
    /// - `ConsumedErr`: recover to sync point, then record error
    /// - `EmptyErr`: convert to `ParseError` and record (no recovery needed — no tokens consumed)
    fn handle_outcome<T>(
        &mut self,
        outcome: ParseOutcome<T>,
        collection: &mut Vec<T>,
        errors: &mut Vec<ParseError>,
        recover: impl FnOnce(&mut Self),
    ) {
        match outcome {
            ParseOutcome::ConsumedOk { value } | ParseOutcome::EmptyOk { value } => {
                collection.push(value);
            }
            ParseOutcome::ConsumedErr { error, .. } => {
                recover(self);
                errors.push(error);
            }
            ParseOutcome::EmptyErr { expected, position } => {
                errors.push(ParseError::from_expected_tokens(&expected, position));
            }
        }
    }

    /// Parse a module (collection of function definitions and tests).
    ///
    /// Uses progress-aware parsing for improved error recovery:
    /// - If parsing fails without progress (no tokens consumed), we skip unknown tokens
    /// - If parsing fails with progress (tokens consumed), we synchronize to a recovery point
    pub fn parse_module(mut self) -> ParseOutput {
        debug!(
            token_count = self.cursor.token_count(),
            "parse_module start"
        );
        let mut module = Module::with_capacity_hint(self.estimated_source_len());
        let mut errors = Vec::new();

        // File-level attribute must appear before imports and declarations.
        // Grammar: source_file = [ file_attribute ] { import } { declaration } .
        module.file_attr = self.parse_file_attribute(&mut errors);

        self.parse_imports(&mut module, &mut errors);

        // Parse declarations (functions, tests, traits, impls, types, etc.)
        while !self.cursor.is_at_end() {
            self.cursor.skip_newlines();
            if self.cursor.is_at_end() {
                break;
            }

            let attrs = self.parse_attributes(&mut errors);
            let visibility = if self.cursor.check(&TokenKind::Pub) {
                self.cursor.advance();
                Visibility::Public
            } else {
                Visibility::Private
            };

            self.dispatch_declaration(attrs, visibility, &mut module, &mut errors);
        }

        // Drain deferred errors/warnings from sub-parsers.
        errors.append(&mut self.deferred_errors);
        let warnings = self.deferred_warnings;

        ParseOutput {
            module,
            arena: SharedArena::new(self.arena),
            errors,
            warnings,
            // Note: For metadata support, use parse_with_metadata() which
            // overwrites this with lexer-captured metadata
            metadata: ModuleExtra::new(),
        }
    }

    /// Parse the import block at the top of a module.
    ///
    /// Imports must appear at the beginning of the file per spec.
    /// Parses `use`, `pub use`, `extension`, and `pub extension` statements.
    fn parse_imports(&mut self, module: &mut Module, errors: &mut Vec<ParseError>) {
        while !self.cursor.is_at_end() {
            self.cursor.skip_newlines();
            if self.cursor.is_at_end() {
                break;
            }

            let is_pub_use = self.cursor.check(&TokenKind::Pub)
                && matches!(self.cursor.peek_next_kind(), TokenKind::Use);

            let is_pub_extension = self.cursor.check(&TokenKind::Pub)
                && matches!(self.cursor.peek_next_kind(), TokenKind::Extension);

            if self.cursor.check(&TokenKind::Use) || is_pub_use {
                let visibility = if is_pub_use {
                    self.cursor.advance();
                    Visibility::Public
                } else {
                    Visibility::Private
                };
                let outcome = self.parse_use(visibility);
                self.handle_outcome(
                    outcome,
                    &mut module.imports,
                    errors,
                    Self::recover_to_next_statement,
                );
            } else if self.cursor.check(&TokenKind::Extension) || is_pub_extension {
                let visibility = if is_pub_extension {
                    self.cursor.advance();
                    Visibility::Public
                } else {
                    Visibility::Private
                };
                let outcome = self.parse_extension_import(visibility);
                self.handle_outcome(
                    outcome,
                    &mut module.extension_imports,
                    errors,
                    Self::recover_to_next_statement,
                );
            } else {
                break;
            }
        }
    }

    /// Dispatch a single top-level declaration.
    ///
    /// Handles all declaration kinds: functions, tests, traits, impls,
    /// extends, type declarations, constants, and error cases (misplaced
    /// imports, orphaned attributes, unknown tokens).
    ///
    /// Returns `true` if a token was consumed (used by the caller to
    /// detect infinite loops).
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive top-level declaration kind dispatch"
    )]
    fn dispatch_declaration(
        &mut self,
        attrs: ParsedAttrs,
        visibility: Visibility,
        module: &mut Module,
        errors: &mut Vec<ParseError>,
    ) {
        if self.cursor.check(&TokenKind::At) {
            let outcome = self.parse_function_or_test(attrs, visibility);
            match outcome {
                ParseOutcome::ConsumedOk { value } | ParseOutcome::EmptyOk { value } => match value
                {
                    FunctionOrTest::Function(func) => module.functions.push(func),
                    FunctionOrTest::Test(test) => module.tests.push(test),
                },
                ParseOutcome::ConsumedErr { error, .. } => {
                    self.recover_to_function();
                    errors.push(error);
                }
                ParseOutcome::EmptyErr { expected, position } => {
                    errors.push(ParseError::from_expected_tokens(&expected, position));
                }
            }
        } else if self.cursor.check(&TokenKind::Trait) {
            let outcome = self.parse_trait(visibility);
            self.handle_outcome(
                outcome,
                &mut module.traits,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Def)
            && matches!(self.cursor.peek_next_kind(), TokenKind::Impl)
        {
            let outcome = self.parse_def_impl(visibility);
            self.handle_outcome(
                outcome,
                &mut module.def_impls,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Impl) {
            let outcome = self.parse_impl();
            self.handle_outcome(
                outcome,
                &mut module.impls,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Extend) {
            let outcome = self.parse_extend();
            self.handle_outcome(
                outcome,
                &mut module.extends,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Type) {
            let outcome = self.parse_type_decl(attrs, visibility);
            self.handle_outcome(
                outcome,
                &mut module.types,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Let) {
            // `let $name = value` — constant declaration (spec §04-constants)
            self.cursor.advance(); // consume `let`
            if self.cursor.check(&TokenKind::Dollar) {
                let outcome = self.parse_const(visibility);
                self.handle_outcome(
                    outcome,
                    &mut module.consts,
                    errors,
                    Self::recover_to_function,
                );
            } else {
                // `let name` without `$` — module-level bindings must be immutable
                errors.push(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "module-level bindings must be immutable".to_string(),
                        self.cursor.current_span(),
                    )
                    .with_help(
                        "Use `let $name = value` with the `$` prefix for module-level constants"
                            .to_string(),
                    ),
                );
                self.recover_to_function();
            }
        } else if self.cursor.check(&TokenKind::Dollar) {
            // Also accept `$name = value` without `let` for backwards compatibility
            let outcome = self.parse_const(visibility);
            self.handle_outcome(
                outcome,
                &mut module.consts,
                errors,
                Self::recover_to_function,
            );
        } else if self.cursor.check(&TokenKind::Extern) {
            let outcome = self.parse_extern_block(visibility);
            self.handle_outcome(
                outcome,
                &mut module.extern_blocks,
                errors,
                Self::recover_to_function,
            );
        } else {
            self.handle_declaration_error(&attrs, errors);
        }
    }

    /// Handle error cases in declaration dispatch.
    ///
    /// Covers: misplaced imports, lexer error tokens, reserved keywords
    /// (`return`), foreign keywords (`fn`, `func`, etc.), orphaned attributes,
    /// and unknown tokens at module level.
    fn handle_declaration_error(&mut self, attrs: &ParsedAttrs, errors: &mut Vec<ParseError>) {
        if self.cursor.check(&TokenKind::Use) || self.cursor.check(&TokenKind::Extension) {
            // Import or extension import after declarations
            errors.push(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "import statements must appear at the beginning of the file",
                self.cursor.current_span(),
            ));
            // Skip the entire import statement to avoid infinite loop
            self.cursor.advance();
            while !self.cursor.is_at_end()
                && !self.cursor.check(&TokenKind::At)
                && !self.cursor.check(&TokenKind::Trait)
                && !self.cursor.check(&TokenKind::Impl)
                && !self.cursor.check(&TokenKind::Type)
                && !self.cursor.check(&TokenKind::Use)
                && !self.cursor.check(&TokenKind::Extension)
            {
                self.cursor.advance();
            }
        } else if self.cursor.current_tag() == TokenKind::TAG_ERROR {
            // Error tokens from the lexer — skip without emitting a parse error.
            // The real diagnostic was already emitted by the lex error pipeline.
            self.cursor.advance();
        } else if self.cursor.check(&TokenKind::Return) {
            // `return` is reserved so users get a targeted error, not "unexpected identifier"
            let kind = error::ParseErrorKind::UnsupportedKeyword {
                keyword: TokenKind::Return,
                reason: "Ori is expression-based: the last expression in a block is its value",
            };
            errors.push(ParseError::from_kind(&kind, self.cursor.current_span()));
            self.cursor.advance();
        } else if self.cursor.current_tag() == TokenKind::TAG_IDENT {
            // Check for foreign keywords from other languages at declaration position.
            // e.g., `fn main()` → "use `@name (params) -> type = body` in Ori"
            if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                let ident_str = self.cursor.interner().lookup(name);
                if let Some(suggestion) = crate::foreign_keywords::lookup_foreign_keyword(ident_str)
                {
                    errors.push(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            format!("`{ident_str}` is not an Ori keyword"),
                            self.cursor.current_span(),
                        )
                        .with_help(String::from(suggestion)),
                    );
                    self.cursor.advance();
                    return;
                }
            }
            // Not a foreign keyword — emit error for unexpected identifier
            if attrs.is_empty() {
                let kind = error::ParseErrorKind::ExpectedDeclaration {
                    found: self.cursor.current_kind().clone(),
                };
                errors.push(ParseError::from_kind(&kind, self.cursor.current_span()));
            } else {
                errors.push(ParseError::new(
                    ori_diagnostic::ErrorCode::E1006,
                    "attributes must be followed by a function or test definition",
                    self.cursor.current_span(),
                ));
            }
            self.cursor.advance();
        } else if !attrs.is_empty() {
            // Attributes without a following function/test
            errors.push(ParseError::new(
                ori_diagnostic::ErrorCode::E1006,
                "attributes must be followed by a function or test definition",
                self.cursor.current_span(),
            ));
            self.cursor.advance();
        } else {
            // Unknown token at module level — not a valid declaration start
            let kind = error::ParseErrorKind::ExpectedDeclaration {
                found: self.cursor.current_kind().clone(),
            };
            errors.push(ParseError::from_kind(&kind, self.cursor.current_span()));
            self.cursor.advance();
        }
    }

    /// Consume a trailing semicolon if present.
    ///
    /// Used after items like `use`, `capset`, and trait method signatures
    /// where `;` terminates the declaration but is not enforced by the parser.
    pub(crate) fn eat_optional_semicolon(&mut self) {
        if self.cursor.check(&TokenKind::Semicolon) {
            self.cursor.advance();
        }
    }

    /// Consume a required trailing semicolon after an item with an expression body.
    ///
    /// Per grammar: function/test/method bodies that end with `}` (block body)
    /// don't need a trailing `;`. Non-block bodies (e.g., `@f () -> int = 42;`)
    /// require `;` to terminate the declaration.
    pub(crate) fn eat_optional_item_semicolon(&mut self) {
        if self.cursor.check(&TokenKind::Semicolon) {
            self.cursor.advance();
        } else if !self.cursor.previous_non_newline_is_rbrace() {
            self.deferred_errors.push(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1016,
                    "expected `;` after item declaration",
                    self.cursor.current_span(),
                )
                .with_help("Block bodies ending with `}` don't need `;`, but expression bodies do"),
            );
        }
    }

    /// Recovery: skip to next statement (@ or use or EOF)
    fn recover_to_next_statement(&mut self) {
        recovery::synchronize(&mut self.cursor, recovery::STMT_BOUNDARY);
    }

    fn recover_to_function(&mut self) {
        recovery::synchronize(&mut self.cursor, recovery::FUNCTION_BOUNDARY);
    }

    /// Parse a module with incremental reuse from a previous parse.
    ///
    /// This method attempts to reuse unchanged declarations from the old AST,
    /// only re-parsing declarations that overlap with the text change.
    fn parse_module_incremental(
        mut self,
        mut state: incremental::IncrementalState<'_>,
        old_arena: &ExprArena,
    ) -> ParseOutput {
        use incremental::{AstCopier, DeclKind};

        let mut module = Module::with_capacity_hint(self.estimated_source_len());
        let mut errors = Vec::new();

        // Imports always get re-parsed since they affect resolution
        self.parse_imports(&mut module, &mut errors);

        // Parse remaining declarations with potential reuse
        while !self.cursor.is_at_end() {
            self.cursor.skip_newlines();
            if self.cursor.is_at_end() {
                break;
            }

            let pos = self.cursor.current_span().start;

            // Try to find a reusable declaration at this position
            if let Some(decl_ref) = state.cursor.find_at(pos) {
                // Check if this declaration is outside the change region
                if !state.cursor.marker().intersects(decl_ref.span) {
                    let copier = AstCopier::new(old_arena, state.cursor.marker().clone());

                    match decl_ref.kind {
                        DeclKind::Function => {
                            let old_func = &state.cursor.module().functions[decl_ref.index];
                            let new_func = copier.copy_function(old_func, &mut self.arena);
                            module.functions.push(new_func);
                        }
                        DeclKind::Test => {
                            let old_test = &state.cursor.module().tests[decl_ref.index];
                            let new_test = copier.copy_test(old_test, &mut self.arena);
                            module.tests.push(new_test);
                        }
                        DeclKind::Type => {
                            let old_type = &state.cursor.module().types[decl_ref.index];
                            let new_type = copier.copy_type_decl(old_type, &mut self.arena);
                            module.types.push(new_type);
                        }
                        DeclKind::Trait => {
                            let old_trait = &state.cursor.module().traits[decl_ref.index];
                            let new_trait = copier.copy_trait(old_trait, &mut self.arena);
                            module.traits.push(new_trait);
                        }
                        DeclKind::Impl => {
                            let old_impl = &state.cursor.module().impls[decl_ref.index];
                            let new_impl = copier.copy_impl(old_impl, &mut self.arena);
                            module.impls.push(new_impl);
                        }
                        DeclKind::DefImpl => {
                            let old_def_impl = &state.cursor.module().def_impls[decl_ref.index];
                            let new_def_impl = copier.copy_def_impl(old_def_impl, &mut self.arena);
                            module.def_impls.push(new_def_impl);
                        }
                        DeclKind::Extend => {
                            let old_extend = &state.cursor.module().extends[decl_ref.index];
                            let new_extend = copier.copy_extend(old_extend, &mut self.arena);
                            module.extends.push(new_extend);
                        }
                        DeclKind::Const => {
                            let old_const = &state.cursor.module().consts[decl_ref.index];
                            let new_const = copier.copy_const(old_const, &mut self.arena);
                            module.consts.push(new_const);
                        }
                        DeclKind::ExtensionImport => {
                            let old_ext = &state.cursor.module().extension_imports[decl_ref.index];
                            let new_ext = copier.copy_extension_import(old_ext);
                            module.extension_imports.push(new_ext);
                        }
                        DeclKind::ExternBlock => {
                            let old_block = &state.cursor.module().extern_blocks[decl_ref.index];
                            let new_block = copier.copy_extern_block(old_block, &mut self.arena);
                            module.extern_blocks.push(new_block);
                        }
                        DeclKind::Import => {
                            unreachable!("imports should not appear in declaration list");
                        }
                    }

                    state.stats.reused_count += 1;
                    self.skip_to_span_end(decl_ref.span);
                    // Consume trailing `;` that was eaten by the original parse
                    // but not included in the declaration span.
                    self.eat_optional_semicolon();
                    continue;
                }
            }

            // Cannot reuse: parse fresh
            state.stats.reparsed_count += 1;

            let attrs = self.parse_attributes(&mut errors);
            let visibility = if self.cursor.check(&TokenKind::Pub) {
                self.cursor.advance();
                Visibility::Public
            } else {
                Visibility::Private
            };

            self.dispatch_declaration(attrs, visibility, &mut module, &mut errors);
        }

        // Drain deferred errors/warnings from sub-parsers.
        errors.append(&mut self.deferred_errors);
        let warnings = self.deferred_warnings;

        ParseOutput {
            module,
            arena: SharedArena::new(self.arena),
            errors,
            warnings,
            // Note: Incremental metadata merging not yet implemented.
            // For now, caller should re-lex with lex_with_comments() and
            // pass to parse_with_metadata() for full metadata support.
            metadata: ModuleExtra::new(),
        }
    }

    /// Skip tokens until we're past the given span end.
    ///
    /// Used during incremental parsing to skip over reused declarations.
    fn skip_to_span_end(&mut self, span: Span) {
        // Adjust the span end for the change delta to get the new end position
        let adjusted_end = self.cursor.current_span().start.max(span.end);

        while !self.cursor.is_at_end() && self.cursor.current_span().start < adjusted_end {
            self.cursor.advance();
        }
    }
}

/// Output from parsing a module, containing the module, arena, and any errors.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseOutput {
    pub module: Module,
    pub arena: SharedArena,
    pub errors: Vec<ParseError>,
    /// Non-fatal warnings (e.g., detached doc comments).
    pub warnings: Vec<ParseWarning>,
    /// Non-semantic metadata for formatting and IDE support.
    ///
    /// Contains comments, blank line positions, and other trivia
    /// that enables lossless roundtrip formatting.
    pub metadata: ModuleExtra,
}

impl ParseOutput {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    // --- Post-parse analysis ---

    /// Generate warnings for detached doc comments.
    ///
    /// Call this after parsing to populate the warnings field with any
    /// doc comments that aren't attached to declarations.
    pub fn check_detached_doc_comments(&mut self) {
        // Collect all declaration start positions
        let mut decl_starts: Vec<u32> = Vec::new();

        for func in &self.module.functions {
            decl_starts.push(func.span.start);
        }
        for test in &self.module.tests {
            decl_starts.push(test.span.start);
        }
        for typ in &self.module.types {
            decl_starts.push(typ.span.start);
        }
        for trait_def in &self.module.traits {
            decl_starts.push(trait_def.span.start);
        }
        for impl_def in &self.module.impls {
            decl_starts.push(impl_def.span.start);
        }
        for ext_import in &self.module.extension_imports {
            decl_starts.push(ext_import.span.start);
        }

        // Sort for binary search efficiency (though unattached_doc_comments does linear scan)
        decl_starts.sort_unstable();

        // Find unattached doc comments
        let unattached = self.metadata.unattached_doc_comments(&decl_starts);

        for comment in unattached {
            // Determine why it's detached
            let reason = if decl_starts.is_empty() {
                DetachmentReason::NoFollowingDeclaration
            } else {
                // Find next declaration after this comment
                let next_decl = decl_starts.iter().find(|&&start| start > comment.span.end);

                match next_decl {
                    Some(&decl_start) => {
                        if self
                            .metadata
                            .has_blank_line_between(comment.span.end, decl_start)
                        {
                            DetachmentReason::BlankLine
                        } else if self
                            .metadata
                            .has_comment_between(comment.span.end, decl_start)
                        {
                            DetachmentReason::RegularCommentInterrupting
                        } else {
                            DetachmentReason::TooFarFromDeclaration
                        }
                    }
                    None => DetachmentReason::NoFollowingDeclaration,
                }
            };

            self.warnings
                .push(ParseWarning::detached_doc_comment(comment.span, reason));
        }
    }
}

/// Parse tokens into a module.
///
/// This is the basic parsing function that doesn't preserve formatting metadata.
/// For formatters and IDEs, use [`parse_with_metadata`] instead.
pub fn parse(tokens: &TokenList, interner: &StringInterner) -> ParseOutput {
    let parser = Parser::new(tokens, interner);
    parser.parse_module()
}

/// Parse tokens with full metadata preservation.
///
/// This function takes tokens and pre-collected metadata from the lexer,
/// producing a `ParseOutput` with full formatting information. Use this for:
/// - Formatters (lossless roundtrip)
/// - IDEs (doc comment display)
/// - Tooling that needs comment information
///
/// # Usage
///
/// Call [`ori_lexer::lex_with_comments`] first, then convert to metadata:
///
/// ```ignore
/// let lex_output = ori_lexer::lex_with_comments(source, &interner);
/// let metadata = lex_output.into_metadata();
/// let parse_output = ori_parse::parse_with_metadata(&lex_output.tokens, metadata, &interner);
///
/// // Access comments attached to declarations
/// let docs = parse_output.metadata.doc_comments_for(fn_start);
/// ```
pub fn parse_with_metadata(
    tokens: &TokenList,
    metadata: ModuleExtra,
    interner: &StringInterner,
) -> ParseOutput {
    let parser = Parser::new(tokens, interner);
    let mut output = parser.parse_module();

    // Transfer metadata from lexer
    output.metadata = metadata;

    output
}

/// Parse tokens with incremental reuse from a previous parse result.
///
/// Uses the old AST to reuse unchanged declarations, only re-parsing
/// those that overlap with the text change. This can provide significant
/// speedups for IDE scenarios where only small edits are made.
///
/// # Arguments
///
/// * `tokens` - The new token list after the edit
/// * `interner` - String interner (must be the same instance used for old result)
/// * `old_result` - The previous parse result to reuse from
/// * `change` - Description of the text change
///
/// # Returns
///
/// A new `ParseOutput` with reused declarations having adjusted spans.
pub fn parse_incremental(
    tokens: &TokenList,
    interner: &StringInterner,
    old_result: &ParseOutput,
    change: ori_ir::incremental::TextChange,
) -> ParseOutput {
    use incremental::{IncrementalState, SyntaxCursor};
    use ori_ir::incremental::ChangeMarker;

    // Find the token before the change for lookahead safety
    let prev_token_end = find_token_end_before(tokens, change.start);

    // Create the change marker with extended region
    let marker = ChangeMarker::from_change(&change, prev_token_end);

    // Create syntax cursor for navigating old AST
    let cursor = SyntaxCursor::new(&old_result.module, &old_result.arena, marker);

    // Create incremental state
    let state = IncrementalState::new(cursor);

    // Parse with incremental reuse
    let parser = Parser::new(tokens, interner);
    parser.parse_module_incremental(state, &old_result.arena)
}

/// Find the end position of the token that ends before `pos`.
///
/// This is used to determine how far back to extend the change region
/// for lookahead safety. Returns 0 if no token ends before `pos`.
///
/// Uses binary search (O(log n)) since tokens are sorted by span position.
fn find_token_end_before(tokens: &TokenList, pos: u32) -> u32 {
    let slice = tokens.as_slice();
    let idx = slice.partition_point(|t| t.span.start < pos);
    if idx > 0 {
        slice[idx - 1].span.end
    } else {
        0
    }
}
