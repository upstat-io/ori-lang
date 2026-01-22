//! Parser for Sigil that produces a flattened AST.
//!
//! This is a recursive descent parser split by responsibility:
//! - `items.rs` - Top-level item parsing (functions, configs, types)
//! - `expr.rs` - Expression parsing (operators, calls, lambdas)
//! - `patterns.rs` - Match pattern parsing
//! - `types.rs` - Type expression parsing

mod items;
mod expr;
mod patterns;
mod types;

use crate::intern::{Name, StringInterner};
use crate::errors::Diagnostic;
use super::{
    Token, TokenKind, TokenList, Span,
    ExprArena, Expr, ExprKind, ExprId,
    items::{Item, Import},
};

/// Parser state.
pub struct Parser<'src, 'i> {
    /// Token list from lexer.
    pub(crate) tokens: &'src TokenList,
    /// String interner.
    pub(crate) interner: &'i StringInterner,
    /// Expression arena.
    pub(crate) arena: ExprArena,
    /// Current token index.
    pub(crate) pos: usize,
    /// Collected diagnostics.
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Collected imports.
    pub(crate) imports: Vec<Import>,
    /// Collected items.
    pub(crate) items: Vec<Item>,
    /// Pending '>' tokens from split '>>' or '>>>'.
    /// Used to handle nested generics like `Option<Option<int>>`.
    pub(crate) pending_gt: usize,
}

impl<'src, 'i> Parser<'src, 'i> {
    /// Create a new parser.
    pub fn new(tokens: &'src TokenList, interner: &'i StringInterner) -> Self {
        Parser {
            tokens,
            interner,
            arena: ExprArena::new(),
            pos: 0,
            diagnostics: Vec::new(),
            imports: Vec::new(),
            items: Vec::new(),
            pending_gt: 0,
        }
    }

    /// Parse a complete module.
    pub fn parse_module(mut self) -> ParseResult {
        self.skip_newlines();

        while !self.at_end() {
            match self.parse_item() {
                Ok(item) => self.items.push(item),
                Err(diag) => {
                    self.diagnostics.push(diag);
                    self.recover_to_next_item();
                }
            }
            self.skip_newlines();
        }

        ParseResult {
            items: self.items,
            imports: self.imports,
            arena: self.arena,
            diagnostics: self.diagnostics,
        }
    }

    /// Parse a single expression (for REPL/testing).
    pub fn parse_expression(mut self) -> (ExprId, ExprArena, Vec<Diagnostic>) {
        self.skip_newlines();
        let expr = match self.expression() {
            Ok(id) => id,
            Err(diag) => {
                self.diagnostics.push(diag);
                self.arena.alloc(Expr::new(ExprKind::Error, self.current_span()))
            }
        };
        (expr, self.arena, self.diagnostics)
    }

    // ===== Token access =====

    pub(crate) fn current(&self) -> &Token {
        &self.tokens.tokens[self.pos.min(self.tokens.tokens.len() - 1)]
    }

    pub(crate) fn current_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    pub(crate) fn current_span(&self) -> Span {
        self.current().span
    }

    pub(crate) fn at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    #[allow(dead_code)]
    pub(crate) fn peek(&self, offset: usize) -> &TokenKind {
        let idx = (self.pos + offset).min(self.tokens.tokens.len() - 1);
        &self.tokens.tokens[idx].kind
    }

    pub(crate) fn advance(&mut self) -> &Token {
        let _token = self.current();
        if !self.at_end() {
            self.pos += 1;
        }
        &self.tokens.tokens[self.pos - 1]
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.current_kind()) == std::mem::discriminant(kind)
    }

    pub(crate) fn consume(&mut self, kind: &TokenKind, msg: &str) -> Result<&Token, Diagnostic> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(msg))
        }
    }

    /// Consume a '>' in type context, handling '>>' as two '>' tokens.
    /// This is needed for nested generics like `Option<Option<int>>`.
    pub(crate) fn consume_gt_in_type(&mut self) -> Result<(), Diagnostic> {
        // First check if we have pending '>' from a previous '>>' split
        if self.pending_gt > 0 {
            self.pending_gt -= 1;
            return Ok(());
        }

        match self.current_kind() {
            TokenKind::Gt => {
                self.advance();
                Ok(())
            }
            TokenKind::Shr => {
                // '>>' - consume it and add one pending '>'
                self.advance();
                self.pending_gt = 1;
                Ok(())
            }
            _ => Err(self.error("expected '>'")),
        }
    }

    pub(crate) fn skip_newlines(&mut self) {
        while matches!(self.current_kind(), TokenKind::Newline) {
            self.advance();
        }
    }

    pub(crate) fn error(&self, msg: &str) -> Diagnostic {
        Diagnostic::error(msg.to_string(), self.current_span())
    }

    fn recover_to_next_item(&mut self) {
        while !self.at_end() {
            match self.current_kind() {
                TokenKind::At | TokenKind::Dollar | TokenKind::Type |
                TokenKind::Pub | TokenKind::Use | TokenKind::Trait |
                TokenKind::Impl | TokenKind::Extend => break,
                TokenKind::Newline => {
                    self.advance();
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ===== Helper parsers =====

    /// Skip a test body without parsing it (for skipped tests with unsupported syntax).
    /// Returns a placeholder expression.
    pub(crate) fn skip_test_body(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        let mut depth = 0;

        // Track nested parens/brackets/braces
        // Stop when we see an item-starting token at depth 0
        loop {
            match self.current_kind() {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                    depth += 1;
                    self.advance();
                }
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    if depth == 0 {
                        // Unexpected close bracket at depth 0 - stop
                        break;
                    }
                    depth -= 1;
                    self.advance();
                    // If we just closed the outermost group, we're done
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::Eof => {
                    break;
                }
                // Item-starting tokens - if at depth 0, we've gone past the body
                TokenKind::At | TokenKind::Dollar | TokenKind::Pub |
                TokenKind::Type | TokenKind::Use | TokenKind::Trait |
                TokenKind::Impl | TokenKind::HashBracket if depth == 0 => {
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }

        // Create a placeholder void expression
        let span = start.merge(self.current_span());
        Ok(self.arena.alloc(Expr::new(ExprKind::Unit, span)))
    }

    pub(crate) fn parse_name(&mut self) -> Result<Name, Diagnostic> {
        match self.current_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            // Allow pattern keywords and other contextual keywords as identifiers
            TokenKind::Map | TokenKind::Filter | TokenKind::Fold |
            TokenKind::Run | TokenKind::Try | TokenKind::Find |
            TokenKind::Collect | TokenKind::Recurse | TokenKind::Parallel |
            TokenKind::Timeout | TokenKind::Retry | TokenKind::Cache |
            TokenKind::Validate | TokenKind::Where | TokenKind::Match => {
                let name = self.interner.intern(self.current_kind().display_name());
                self.advance();
                Ok(name)
            }
            _ => Err(self.error("expected identifier")),
        }
    }
}

/// Result of parsing a module.
pub struct ParseResult {
    /// Top-level items.
    pub items: Vec<Item>,
    /// Import declarations.
    pub imports: Vec<Import>,
    /// Expression arena.
    pub arena: ExprArena,
    /// Parse diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}
