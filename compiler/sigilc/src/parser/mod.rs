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
mod patterns;
mod types_parse;

#[cfg(test)]
mod tests;

use crate::ast::*;
use crate::lexer::{SpannedToken, Token};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub(super) fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.value)
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

    pub(super) fn expect(&mut self, expected: Token) -> Result<&SpannedToken, String> {
        match self.current() {
            Some(t) if *t == expected => Ok(self.advance().unwrap()),
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
                    _ => Err("Expected @, type, or use after pub".to_string()),
                }
            }
            Some(Token::Type) => self.parse_type_def(false).map(Item::TypeDef),
            Some(Token::Use) => self.parse_use().map(Item::Use),
            Some(t) => Err(format!("Unexpected token at top level: {:?}", t)),
            None => Err("Unexpected end of input".to_string()),
        }
    }
}

pub fn parse(tokens: Vec<SpannedToken>, filename: &str) -> Result<Module, String> {
    let mut parser = Parser::new(tokens);
    parser.parse_module(filename)
}
