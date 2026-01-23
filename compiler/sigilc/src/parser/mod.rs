// Parser for Sigil
// Converts tokens into AST
//
// The parser is split into several submodules:
// - expr.rs: Expression parsing
// - items.rs: Item parsing (functions, tests, configs, types, use)
// - types_parse.rs: Type expression parsing
// - patterns.rs: Pattern and match expression parsing

mod expr;
mod items;
mod operators;
mod patterns;
mod postfix;
mod primary;
mod types_parse;

#[cfg(test)]
mod tests;

use crate::ast::*;
use crate::lexer::{SpannedToken, Token};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    /// The end position of the source (for handling EOF spans)
    source_len: usize,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        // Calculate source length from the last token's end position
        let source_len = tokens.last().map(|t| t.span.end).unwrap_or(0);
        Parser { tokens, pos: 0, source_len }
    }

    pub(super) fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.value)
    }

    /// Get the current token with its span
    pub(super) fn current_spanned(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }

    pub(super) fn peek(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset).map(|t| &t.value)
    }

    pub(super) fn advance(&mut self) -> Option<&SpannedToken> {
        if self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    /// Get the start position for the current token (or end of source if at EOF)
    pub(super) fn current_start(&self) -> usize {
        self.tokens.get(self.pos)
            .map(|t| t.span.start)
            .unwrap_or(self.source_len)
    }

    /// Get the end position of the previous token (or 0 if at start)
    pub(super) fn previous_end(&self) -> usize {
        if self.pos > 0 {
            self.tokens.get(self.pos - 1)
                .map(|t| t.span.end)
                .unwrap_or(0)
        } else {
            0
        }
    }

    /// Create a span from start to the end of the previous token
    pub(super) fn make_span(&self, start: usize) -> Span {
        start..self.previous_end()
    }

    /// Wrap an expression with a span
    pub(super) fn spanned(&self, expr: Expr, start: usize) -> SpannedExpr {
        SpannedExpr::new(expr, self.make_span(start))
    }

    #[allow(clippy::expect_used)] // advance() cannot return None after current() returned Some
    pub(super) fn expect(&mut self, expected: Token) -> Result<&SpannedToken, String> {
        match self.current() {
            Some(t) if *t == expected => Ok(self.advance().expect("current() returned Some")),
            Some(t) => Err(format!("Expected {:?}, found {:?}", expected, t)),
            None => Err(format!("Expected {:?}, found end of input", expected)),
        }
    }

    pub(super) fn skip_newlines(&mut self) {
        while matches!(self.current(), Some(Token::Newline)) {
            self.advance();
        }
    }

    /// Try to get an identifier from the current token.
    /// This treats certain keywords as valid identifiers for context-sensitive parsing.
    /// Used for function names after @ where keywords like type names should be valid.
    pub(super) fn try_get_ident(&self) -> Option<String> {
        match self.current()? {
            Token::Ident(n) => Some(n.clone()),
            // Type keywords - valid as function names
            Token::IntType => Some("int".to_string()),
            Token::FloatType => Some("float".to_string()),
            Token::StrType => Some("str".to_string()),
            Token::BoolType => Some("bool".to_string()),
            // Other keywords that could be function names
            Token::Assert => Some("assert".to_string()),
            Token::AssertErr => Some("assert_err".to_string()),
            Token::Tests => Some("tests".to_string()),
            _ => None,
        }
    }

    pub fn parse_module(&mut self, name: &str) -> Result<Module, String> {
        let mut items = Vec::new();

        self.skip_newlines();

        while self.current().is_some() {
            self.skip_newlines();
            if self.current().is_none() {
                break;
            }

            let item = self.parse_item()?;
            items.push(item);

            self.skip_newlines();
        }

        Ok(Module {
            name: name.to_string(),
            items,
        })
    }

    fn parse_item(&mut self) -> Result<Item, String> {
        match self.current() {
            Some(Token::Dollar) => self.parse_config().map(Item::Config),
            Some(Token::At) => self.parse_function_or_test(false),
            Some(Token::Pub) => {
                self.advance();
                match self.current() {
                    Some(Token::At) => self.parse_function_or_test(true),
                    Some(Token::Type) => self.parse_type_def(true).map(Item::TypeDef),
                    Some(Token::Use) => self.parse_use().map(Item::Use),
                    Some(Token::Trait) => self.parse_trait(true).map(Item::Trait),
                    Some(Token::Impl) => self.parse_impl().map(Item::Impl),
                    _ => Err("Expected @, type, use, trait, or impl after pub".to_string()),
                }
            }
            Some(Token::Type) => self.parse_type_def(false).map(Item::TypeDef),
            Some(Token::Use) => self.parse_use().map(Item::Use),
            Some(Token::Trait) => self.parse_trait(false).map(Item::Trait),
            Some(Token::Impl) => self.parse_impl().map(Item::Impl),
            Some(Token::Extend) => self.parse_extend().map(Item::Extend),
            Some(Token::Extension) => self.parse_extension().map(Item::Extension),
            Some(t) => Err(format!("Unexpected token at top level: {:?}", t)),
            None => Err("Unexpected end of input".to_string()),
        }
    }
}

pub fn parse(tokens: Vec<SpannedToken>, filename: &str) -> Result<Module, String> {
    let mut parser = Parser::new(tokens);
    parser.parse_module(filename)
}
