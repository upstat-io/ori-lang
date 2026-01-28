//! Token cursor for navigating the token stream.
//!
//! Provides low-level token access, lookahead, and consumption methods.

use super::ParseError;
use ori_diagnostic::ErrorCode;
use ori_ir::{Name, Span, StringInterner, Token, TokenKind, TokenList};

/// Cursor for navigating tokens.
///
/// Provides methods for accessing, consuming, and checking tokens
/// during parsing. Tracks current position in the token stream.
pub struct Cursor<'a> {
    tokens: &'a TokenList,
    interner: &'a StringInterner,
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Create a new cursor at the start of the token stream.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        Cursor {
            tokens,
            interner,
            pos: 0,
        }
    }

    /// Get a reference to the string interner.
    pub fn interner(&self) -> &'a StringInterner {
        self.interner
    }

    /// Get the current position in the token stream.
    ///
    /// Used for progress tracking - compare positions before and after
    /// parsing to determine if tokens were consumed.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Get the current token.
    pub fn current(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or(&self.tokens[self.tokens.len() - 1])
    }

    /// Get the current token's kind.
    pub fn current_kind(&self) -> TokenKind {
        self.current().kind.clone()
    }

    /// Get the current token's span.
    pub fn current_span(&self) -> Span {
        self.current().span
    }

    /// Get the previous token's span.
    pub fn previous_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span::DUMMY
        }
    }

    /// Check if at end of token stream.
    pub fn is_at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    /// Check if the current token matches the given kind.
    pub fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.current_kind()) == std::mem::discriminant(kind)
    }

    /// Check if the current token is an identifier.
    pub fn check_ident(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Ident(_))
    }

    /// Check if the current token is a type keyword.
    pub fn check_type_keyword(&self) -> bool {
        matches!(
            self.current_kind(),
            TokenKind::IntType
                | TokenKind::FloatType
                | TokenKind::BoolType
                | TokenKind::StrType
                | TokenKind::CharType
                | TokenKind::ByteType
                | TokenKind::Void
                | TokenKind::NeverType
        )
    }

    /// Peek at the next token's kind (one-token lookahead).
    /// Returns `TokenKind::Eof` if at the end of the stream.
    pub fn peek_next_kind(&self) -> TokenKind {
        if self.pos + 1 < self.tokens.len() {
            self.tokens[self.pos + 1].kind.clone()
        } else {
            TokenKind::Eof
        }
    }

    /// Peek at the next token (one-token lookahead).
    /// Returns the EOF token if at the end of the stream.
    pub fn peek_next_token(&self) -> &Token {
        self.tokens
            .get(self.pos + 1)
            .unwrap_or(&self.tokens[self.tokens.len() - 1])
    }

    /// Get the next token's span.
    pub fn peek_next_span(&self) -> Span {
        self.peek_next_token().span
    }

    /// Check if two spans are adjacent (no whitespace between them).
    /// Returns true if span1.end == span2.start.
    pub fn spans_adjacent(&self, span1: Span, span2: Span) -> bool {
        span1.end == span2.start
    }

    /// Check if current token and next token are adjacent (no whitespace).
    pub fn current_and_next_adjacent(&self) -> bool {
        self.spans_adjacent(self.current_span(), self.peek_next_span())
    }

    /// Check if looking at `>` followed immediately by `>` (no whitespace).
    /// Used for detecting `>>` shift operator in expression context.
    pub fn is_shift_right(&self) -> bool {
        self.check(&TokenKind::Gt)
            && matches!(self.peek_next_kind(), TokenKind::Gt)
            && self.current_and_next_adjacent()
    }

    /// Check if looking at `>` followed immediately by `=` (no whitespace).
    /// Used for detecting `>=` comparison operator in expression context.
    pub fn is_greater_equal(&self) -> bool {
        self.check(&TokenKind::Gt)
            && matches!(self.peek_next_kind(), TokenKind::Eq)
            && self.current_and_next_adjacent()
    }

    /// Consume two adjacent tokens as a compound operator.
    /// Returns the combined span.
    /// Panics if not at the expected tokens.
    pub fn consume_compound(&mut self) -> Span {
        let start = self.current_span();
        self.advance();
        let end = self.current_span();
        self.advance();
        start.merge(end)
    }

    /// Check if the next token (lookahead) is a left paren.
    pub fn next_is_lparen(&self) -> bool {
        self.pos + 1 < self.tokens.len()
            && matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen)
    }

    /// Check if the next token (lookahead) is a colon.
    pub fn next_is_colon(&self) -> bool {
        self.pos + 1 < self.tokens.len()
            && matches!(self.tokens[self.pos + 1].kind, TokenKind::Colon)
    }

    /// Check if this is capability provision syntax: `with Ident =`
    /// Current position should be at `with`.
    pub fn is_with_capability_syntax(&self) -> bool {
        // Need at least 3 tokens ahead: with Ident =
        if self.pos + 2 >= self.tokens.len() {
            return false;
        }
        // Token at pos+1 should be an identifier
        let next_is_ident = matches!(self.tokens[self.pos + 1].kind, TokenKind::Ident(_));
        // Token at pos+2 should be =
        let then_is_eq = matches!(self.tokens[self.pos + 2].kind, TokenKind::Eq);

        next_is_ident && then_is_eq
    }

    /// Check if looking at named argument pattern: identifier followed by colon.
    /// Used to distinguish `name: value` (named arg) from `value` (positional).
    pub fn is_named_arg_start(&self) -> bool {
        let is_ident = matches!(self.current_kind(), TokenKind::Ident(_))
            || self.soft_keyword_to_name().is_some()
            || self.is_keyword_usable_as_name();
        is_ident && self.next_is_colon()
    }

    /// Check if current token is a keyword that can be used as a named argument name.
    fn is_keyword_usable_as_name(&self) -> bool {
        matches!(
            self.current_kind(),
            TokenKind::Where
                | TokenKind::Match
                | TokenKind::For
                | TokenKind::In
                | TokenKind::If
                | TokenKind::Type
                | TokenKind::Parallel
                | TokenKind::Timeout
        )
    }

    /// Check if current token is a built-in I/O primitive that can be used as an identifier.
    /// These are only treated as keywords when followed by `(`.
    /// Returns the interned name if it's a soft keyword, None otherwise.
    pub fn soft_keyword_to_name(&self) -> Option<&'static str> {
        match self.current_kind() {
            TokenKind::Print => Some("print"),
            TokenKind::Panic => Some("panic"),
            _ => None,
        }
    }

    /// Advance to the next token and return the consumed token.
    pub fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        &self.tokens[self.pos - 1]
    }

    /// Skip all newline tokens.
    pub fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    /// Expect the current token to be of the given kind, advance and return it.
    /// Returns an error if the token kind doesn't match.
    pub fn expect(&mut self, kind: &TokenKind) -> Result<&Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(ParseError::new(
                ErrorCode::E1001,
                format!("expected {kind:?}, found {:?}", self.current_kind()),
                self.current_span(),
            )
            .with_context(format!("expected {kind:?}")))
        }
    }

    /// Expect and consume an identifier, returning its interned name.
    /// Also accepts soft keywords (len, min, max, etc.) as identifiers.
    pub fn expect_ident(&mut self) -> Result<Name, ParseError> {
        // Accept regular identifiers
        if let TokenKind::Ident(name) = self.current_kind() {
            self.advance();
            Ok(name)
        // Also accept soft keywords as identifiers
        } else if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner.intern(name_str);
            self.advance();
            Ok(name)
        } else {
            Err(ParseError::new(
                ErrorCode::E1004,
                format!("expected identifier, found {:?}", self.current_kind()),
                self.current_span(),
            ))
        }
    }

    /// Accept an identifier or a keyword that can be used as a named argument name.
    /// This handles cases like `where:` in the find pattern where `where` is a keyword.
    pub fn expect_ident_or_keyword(&mut self) -> Result<Name, ParseError> {
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
            // Compiler-supported pattern keywords that can be used as named argument names
            TokenKind::Parallel => {
                self.advance();
                Ok(self.interner.intern("parallel"))
            }
            TokenKind::Timeout => {
                self.advance();
                Ok(self.interner.intern("timeout"))
            }
            _ => Err(ParseError::new(
                ErrorCode::E1004,
                format!(
                    "expected identifier or keyword, found {:?}",
                    self.current_kind()
                ),
                self.current_span(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_navigation() {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex("let x = 42", &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        let mut cursor = Cursor::new(tokens, interner);

        assert!(cursor.check(&TokenKind::Let));
        assert!(!cursor.is_at_end());

        cursor.advance();
        assert!(cursor.check_ident());

        cursor.advance();
        assert!(cursor.check(&TokenKind::Eq));

        cursor.advance();
        assert!(matches!(cursor.current_kind(), TokenKind::Int(_)));

        cursor.advance();
        assert!(cursor.is_at_end());
    }

    #[test]
    fn test_expect_success() {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex("let x", &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        let mut cursor = Cursor::new(tokens, interner);

        let result = cursor.expect(&TokenKind::Let);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expect_failure() {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex("let x", &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        let mut cursor = Cursor::new(tokens, interner);

        let result = cursor.expect(&TokenKind::If);
        assert!(result.is_err());
    }

    #[test]
    fn test_skip_newlines() {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex("let\n\n\nx", &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        let mut cursor = Cursor::new(tokens, interner);

        cursor.advance(); // skip 'let'
        cursor.skip_newlines();
        assert!(cursor.check_ident()); // should be at 'x'
    }

    #[test]
    fn test_lookahead() {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex("foo()", &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        let cursor = Cursor::new(tokens, interner);

        assert!(cursor.next_is_lparen());
    }

    #[test]
    fn test_check_type_keyword() {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex("int float bool str", &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        let mut cursor = Cursor::new(tokens, interner);

        assert!(cursor.check_type_keyword()); // int
        cursor.advance();
        assert!(cursor.check_type_keyword()); // float
        cursor.advance();
        assert!(cursor.check_type_keyword()); // bool
        cursor.advance();
        assert!(cursor.check_type_keyword()); // str
    }
}
